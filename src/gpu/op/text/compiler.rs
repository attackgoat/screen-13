use {
    super::{
        bitmap_font::{BitmapFont, Vertex as BitmapFontVertex},
        command::{BitmapCommand, Command, ScalableCommand},
        instruction::Instruction,
        scalable_font::ScalableFont,
        Font,
    },
    crate::{
        gpu::{
            data::Mapping,
            op::{Allocation, DirtyData, DirtyLruData, Lru, Stride},
            pool::Pool,
            Texture2d,
        },
        math::{CoordF, Extent, Mat4},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{borrow::Borrow, cmp::Ordering, marker::PhantomData, ptr::copy_nonoverlapping},
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[non_exhaustive]
pub enum Asm {
    BeginBitmap,
    BindBitmapBuffer,
    CopyBitmapVertices,
    DrawBitmap(usize),
    TransferBitmapData,
    WriteBitmapVertices,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Char(char);

impl Stride for Char {
    fn stride() -> u64 {
        BitmapFontVertex::STRIDE
    }
}

pub struct Compilation<'a, 'c, C, P, T>
where
    C: Borrow<Command<P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    __: PhantomData<T>,
    cmds: &'c [C],
    compiler: &'a mut Compiler<P>,
    idx: usize,
}

impl<'c, C, P, T> Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub fn bitmap_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler.bitmap_textures.iter().map(|tex| &**tex)
    }

    pub fn bitmap_outline_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler
            .bitmap_outline_textures
            .iter()
            .map(|tex| &**tex)
    }

    // fn bind_texture_descriptors(&self, idx: usize) -> Instruction {
    //     let src = Shared::as_ptr(&self.compiler.cmds[idx].src);
    //     let desc_set = self
    //         .compiler
    //         .textures
    //         .binary_search_by(|probe| Shared::as_ptr(&probe).cmp(&src))
    //         .unwrap();

    //     Instruction::TextureDescriptors(desc_set)
    // }

    // fn write_texture(&self, transform: Mat4) -> Instruction {
    //     Instruction::TextureWrite(transform)
    // }

    /// Returns true if no writes are rendered.
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs:
// https://github.com/rust-lang/rust/issues/44265
impl<'c, C, P, T> Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub(super) fn next(&mut self) -> Option<Instruction<'_, P>> {
        if self.idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        Some(match &self.compiler.code[idx] {
            // Asm::BindTextureDescriptors(idx) => self.bind_texture_descriptors(*idx),
            // Asm::WriteTexture(transform) => self.write_texture(*transform),
            _ => todo!(),
        })
    }
}

impl<'c, C, P, T> Drop for Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    fn drop(&mut self) {
        // Reset non-critical resources
        self.compiler.code.clear();
    }
}

/// Compiles a series of drawing commands into renderable instructions. The purpose of this
/// structure is two-fold:
/// - Reduce per-draw allocations with line and light caches (they are not cleared after each use)
/// - Store references to the in-use mesh textures during rendering (this cache is cleared after
///   use)
pub struct Compiler<P>
where
    P: 'static + SharedPointerKind,
{
    bitmap_fonts: Vec<(Shared<BitmapFont<P>, P>, DirtyLruData<Char, P>)>,
    bitmap_textures: Vec<Shared<Texture2d, P>>,
    bitmap_outline_textures: Vec<Shared<Texture2d, P>>,
    code: Vec<Asm>,
    scalable_fonts: Vec<(Shared<ScalableFont, P>, DirtyLruData<Char, P>)>,
}

impl<P> Compiler<P>
where
    P: SharedPointerKind,
{
    pub fn compile<'a, 'c, C, T>(
        &'a mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmds: &'c mut [C],
        dims: Extent,
    ) -> Compilation<'a, 'c, C, P, T>
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        // Sort by texture: Unstable because we don't claim to offering any ordering within a single
        // batch - submit additional batches to ensure order!
        cmds.sort_unstable_by(|lhs, rhs| match lhs.borrow().font() {
            Font::Bitmap(lhs) => match rhs.borrow().font() {
                Font::Bitmap(rhs) => Shared::as_ptr(&lhs).cmp(&Shared::as_ptr(&rhs)),
                _ => Ordering::Less,
            },
            Font::Scalable(lhs) => match rhs.borrow().font() {
                Font::Scalable(rhs) => Shared::as_ptr(&lhs).cmp(&Shared::as_ptr(&rhs)),
                _ => Ordering::Greater,
            },
        });

        // Make sure we have initialized a dirty data and lru for each font
        {
            let mut last_bitmap_ptr = None;
            let mut last_scalable_ptr = None;
            for (idx, cmd) in cmds.iter().enumerate() {
                match cmd.borrow().font() {
                    Font::Bitmap(font) => {
                        // We are sorted so if we repeat the ptr it's already here
                        let font_ptr = font;//Shared::as_ptr(font);
                        if let Some(ptr) = last_bitmap_ptr {
                            if ptr == font_ptr {
                                continue;
                            }
                        }

                        last_bitmap_ptr = Some(font_ptr);

                        // Ensure we've got the data ready
                        if let Err(idx) = self.bitmap_fonts.binary_search_by(|probe| {
                            Shared::as_ptr(&probe.0).cmp(&Shared::as_ptr(font))
                        }) {
                            self.bitmap_fonts
                                .insert(idx, (Shared::clone(font), Default::default()));
                        }

                        // Compile all uses of this font
                        self.compile_bitmap_font(
                            #[cfg(feature = "debug-names")]
                            name,
                            pool,
                            cmds,
                            dims,
                            idx,
                        );
                    }
                    Font::Scalable(ref font) => {
                        // We are sorted so if we repeat the ptr it's already here
                        let font_ptr = Shared::as_ptr(font);
                        if let Some(ptr) = last_scalable_ptr {
                            if ptr == font_ptr {
                                continue;
                            }
                        }

                        last_scalable_ptr = Some(font_ptr);

                        // Ensure we've got the data ready
                        if let Err(idx) = self.scalable_fonts.binary_search_by(|probe| {
                            Shared::as_ptr(&probe.0).cmp(&Shared::as_ptr(font))
                        }) {
                            self.scalable_fonts
                                .insert(idx, (Shared::clone(font), Default::default()));
                        }
                    },
                }
            }
        }

        // For all commands:
        // - Bitmap: Check each letter
        //           Tesselate into 4 triangle strip vertices
        //           Store vertices in LRU keyed by letter
        //           Put vertices in data, write data cpu->gpu
        //           Hash each letter and transform
        //           First time seeing a letter/position: Draw it, prepare instancing
        //           Second time seeing a letter/position: Instance it

        

        // // PERF: Should hand roll this
        // // Read as:
        // // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
        // // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
        // Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
        //     * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
        //     * Mat4::from_translation(vec3(
        //         self.layout.x / dims.x as f32,
        //         self.layout.y / dims.y as f32,
        //         0.0,
        //     ))

        Compilation {
            __: PhantomData,
            cmds,
            compiler: self,
            idx: 0,
        }
    }

    fn compile_bitmap_font<C, T>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmds: &[C],
        dims: Extent,
        idx: usize,
    ) where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        let bitmap_font = cmds[idx].borrow().font().as_bitmap().unwrap();

        let bitmaps = cmds[idx..].iter().filter(|cmd| match (*cmd).borrow() {
            Command::Position(_) | Command::Transform(_) => true,
            Command::SizePosition(_) | Command::SizeTransform(_) => false,
        });

        // // Allocate enough `buf` to hold everything in the existing cache and everything we could
        // // possibly draw
        // unsafe {
        //     self.bitmap_font.alloc_data(
        //         #[cfg(feature = "debug-names")]
        //         &format!("{} bitmap font vertex buffer", name),
        //         pool,
        //         (self.bitmap_font.lru.len() + text.len()) as u64 * BitmapFontVertex::STRIDE,
        //     );
        // }
        // let buf = self.bitmap_font.buf.as_mut().unwrap();

        // // Copy data from the previous GPU buffer to the new one
        // if buf.data.previous.is_some() {
        //     self.code.push(Asm::TransferBitmapData);
        // }

        // // Copy data from the uncompacted end of the buffer back to linear data
        // buf.compact_cache(&mut self.bitmap_font.lru);
        // if !buf.pending_copies.is_empty() {
        //     self.code.push(Asm::CopyBitmapVertices);
        // }

        // // start..end is the back of the buffer where we push new chars
        // let start = buf
        //     .usage
        //     .last()
        //     .map_or(0, |(offset, _)| offset + BitmapFontVertex::STRIDE);
        // let mut end = start;

        // let write_idx = self.code.len();
        // self.code.push(Asm::BeginBitmap);
        // self.code.push(Asm::BindBitmapBuffer);

        // // First we make sure all characters are in the lru data ...
        // let chars = text.chars().map(|chr| BitmapFontChar(chr));
        // for chr in chars {
        //     match self
        //         .bitmap_font
        //         .lru
        //         .binary_search_by(|probe| probe.key.cmp(&chr))
        //     {
        //         Err(idx) => {
        //             // Cache the vertices for this character
        //             let new_end = end + BitmapFontVertex::STRIDE;
        //             let vertices = &[0u8]; //gen_rect_light(key.dims(), key.range(), key.radius());

        //             unsafe {
        //                 let mut mapped_range =
        //                     buf.data.current.map_range_mut(end..new_end).unwrap();
        //                 copy_nonoverlapping(
        //                     vertices.as_ptr(),
        //                     mapped_range.as_mut_ptr(),
        //                     BitmapFontVertex::STRIDE as _,
        //                 );

        //                 Mapping::flush(&mut mapped_range).unwrap();
        //             }

        //             // Create new cache entries for this rectangular light
        //             buf.usage.push((end, chr));
        //             self.bitmap_font
        //                 .lru
        //                 .insert(idx, Lru::new(chr, end, pool.lru_threshold));
        //             end = new_end;
        //         }
        //         Ok(idx) => {
        //             self.bitmap_font.lru[idx].recently_used = pool.lru_threshold;
        //         }
        //     }
        // }

        // // ... now we can draw them using index
        // self.code.push(Asm::DrawBitmap(idx));

        // // We may need to write these vertices from the CPU to the GPU
        // if start != end {
        //     buf.pending_write = Some(start..end);
        //     self.code.insert(write_idx, Asm::WriteBitmapVertices);
        // }
    }

    fn compile_scalable<L, T>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmd: &ScalableCommand<L, P, T>,
        dims: Extent,
    ) where
        T: AsRef<str>,
    {
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile`
    /// function.
    ///
    /// Must NOT be called before the previously drawn frame is completed.
    pub(super) fn reset(&mut self) {
        // Reset critical resources
        self.bitmap_textures.clear();
        self.bitmap_outline_textures.clear();

        // Advance the least-recently-used caching algorithm one step forward
        for (_, lru) in self.bitmap_fonts.iter_mut() {
            lru.step();
        }

        // Remove any fonts which are no longer in use
        self.bitmap_fonts.retain(|(_, data)| !data.lru.is_empty());
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            bitmap_fonts: Default::default(),
            bitmap_outline_textures: Default::default(),
            bitmap_textures: Default::default(),
            code: Default::default(),
            scalable_fonts: Default::default(),
        }
    }
}
