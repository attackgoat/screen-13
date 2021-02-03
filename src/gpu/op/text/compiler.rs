use {
    super::{
        bitmap_font::{BitmapFont, Vertex as BitmapFontVertex},
        command::{Command, ScalableCommand},
        instruction::Instruction,
        key::{Position, Transform},
        scalable_font::ScalableFont,
        Font,
    },
    crate::{
        gpu::{
            data::Mapping,
            op::{DirtyLruData, Lru, Stride},
            pool::Pool,
            Texture2d,
        },
        math::Extent,
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{
        borrow::Borrow, cmp::Ordering, marker::PhantomData, ops::Range, ptr::copy_nonoverlapping,
    },
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[non_exhaustive]
pub enum Asm {
    BeginBitmap,
    BeginBitmapOutline,
    BeginScalable,
    BindBitmapBuffer,
    CopyBitmapVertices(usize),
    DrawBitmaps(usize),
    TransferBitmapData(usize),
    WriteBitmapVertices,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct CharPosition(char, Position);

impl Stride for CharPosition {
    fn stride() -> u64 {
        BitmapFontVertex::STRIDE
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct CharTransform(char, Transform);

impl Stride for CharTransform {
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

        // Some(match &self.compiler.code[idx] {
        // Asm::BindTextureDescriptors(idx) => self.bind_texture_descriptors(*idx),
        // Asm::WriteTexture(transform) => self.write_texture(*transform),
        // _ => todo!(),
        // })

        todo!();
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
    bitmap_font_positions: Vec<(Shared<BitmapFont<P>, P>, DirtyLruData<CharPosition, P>)>,
    bitmap_font_transforms: Vec<(Shared<BitmapFont<P>, P>, DirtyLruData<CharTransform, P>)>,
    bitmap_textures: Vec<Shared<Texture2d, P>>,
    bitmap_outline_textures: Vec<Shared<Texture2d, P>>,
    code: Vec<Asm>,
    scalable_font_positions: Vec<(Shared<ScalableFont, P>, DirtyLruData<CharPosition, P>)>,
    scalable_font_transforms: Vec<(Shared<ScalableFont, P>, DirtyLruData<CharTransform, P>)>,
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
            let mut prev_bitmap_font = None;
            let mut prev_scalable_font = None;
            let mut idx = 0;
            let len = cmds.len();
            while idx < len {
                let cmd = &cmds[idx];
                match cmd.borrow().font() {
                    Font::Bitmap(font) => {
                        // We are sorted so if we repeat the ptr it's already here
                        let different_font = if let Some(prev_font) = prev_bitmap_font {
                            font != prev_font
                        } else {
                            false
                        };

                        if different_font {
                            prev_bitmap_font = Some(font);

                            // Compile all uses of this font ...
                            idx = self.compile_bitmap_font(
                                #[cfg(feature = "debug-names")]
                                name,
                                pool,
                                cmds,
                                idx,
                            );
                        }

                        // ... now we can draw them using index
                        self.code.push(Asm::DrawBitmaps(idx));

                        // Move to the next font if we didn't do so above
                        if !different_font {
                            idx += 1;
                        }
                    }
                    Font::Scalable(font) => {
                        // We are sorted so if we repeat the ptr it's already here
                        if let Some(pre_font) = prev_scalable_font {
                            if font == pre_font {
                                continue;
                            }
                        }

                        prev_scalable_font = Some(font);

                        // // Ensure we've got the data ready
                        // if let Err(idx) = self.scalable_fonts.binary_search_by(|probe| {
                        //     Shared::as_ptr(&probe.0).cmp(&Shared::as_ptr(font))
                        // }) {
                        //     self.scalable_fonts
                        //         .insert(idx, (Shared::clone(font), Default::default()));
                        // }
                    }
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
        cmd_idx: usize,
    ) -> usize
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        // Find the given "start" command at idx and how many following commands are the same font
        let cmd = cmds[cmd_idx].borrow();
        let font = cmd.font();
        let bitmap_font = font.as_bitmap().unwrap();
        let mut end_idx = cmd_idx + 1;
        for (idx, cmd) in cmds[end_idx..].iter().enumerate() {
            match cmd.borrow() {
                Command::Position(_) | Command::Transform(_) => {
                    end_idx = idx;

                    if bitmap_font != cmd.borrow().font().as_bitmap().unwrap() {
                        break;
                    }
                }
                _ => break,
            }
        }

        self.compile_bitmap_font_positions(
            #[cfg(feature = "debug-names")]
            name,
            pool,
            bitmap_font,
            cmds,
            cmd_idx..end_idx,
        );

        end_idx
    }

    fn compile_bitmap_font_positions<C, T>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        font: &Shared<BitmapFont<P>, P>,
        cmds: &[C],
        range: Range<usize>,
    ) where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        // Figure out the total length of all position texts using this font
        let text_len: usize = cmds[range.clone()]
            .iter()
            .filter(|cmd| (*cmd).borrow().is_position())
            .map(|cmd| (*cmd).borrow().text().len())
            .sum();
        if text_len == 0 {
            return;
        }

        // Ensure we've got the data ready
        let font_idx = match self
            .bitmap_font_positions
            .binary_search_by(|probe| Shared::as_ptr(&probe.0).cmp(&Shared::as_ptr(font)))
        {
            Err(idx) => {
                self.bitmap_font_positions
                    .insert(idx, (Shared::clone(font), Default::default()));
                idx
            }
            Ok(idx) => idx,
        };

        // Allocate enough `buf` to hold everything in the existing cache and everything we could
        // possibly draw (assuming each character is unique)
        let (_, data) = &mut self.bitmap_font_positions[font_idx];
        unsafe {
            data.alloc(
                #[cfg(feature = "debug-names")]
                &format!("{} bitmap font vertex buffer", name),
                pool,
                (data.lru.len() + text_len) as u64 * BitmapFontVertex::STRIDE,
            );
        }
        let buf = data.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferBitmapData(range.start));
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        buf.compact_cache(&mut data.lru, pool.lru_expiry);
        if !buf.pending_copies.is_empty() {
            self.code.push(Asm::CopyBitmapVertices(range.start));
        }

        // start..end is the back of the buffer where we push new chars
        let start = buf
            .usage
            .last()
            .map_or(0, |(offset, _)| offset + BitmapFontVertex::STRIDE);
        let mut end = start;

        // Make sure all characters are in the lru data
        for chr in cmds[range]
            .iter()
            .filter(|cmd| (*cmd).borrow().is_position())
            .flat_map(|cmd| {
                let text = (*cmd).borrow().text();

                text.chars()
            })
            .map(|chr| CharPosition(chr, Position))
        {
            match data.lru.binary_search_by(|probe| probe.key.cmp(&chr)) {
                Err(idx) => {
                    // Cache the vertices for this character
                    let new_end = end + BitmapFontVertex::STRIDE;
                    let vertices = &[0u8]; //gen_rect_light(key.dims(), key.range(), key.radius());

                    unsafe {
                        let mut mapped_range =
                            buf.data.current.map_range_mut(end..new_end).unwrap();
                        copy_nonoverlapping(
                            vertices.as_ptr(),
                            mapped_range.as_mut_ptr(),
                            BitmapFontVertex::STRIDE as _,
                        );

                        Mapping::flush(&mut mapped_range).unwrap();
                    }

                    // Create new cache entries for this rectangular light
                    buf.usage.push((end, chr));
                    data.lru.insert(idx, Lru::new(chr, end, pool.lru_expiry));
                    end = new_end;
                }
                Ok(idx) => {
                    data.lru[idx].expiry = pool.lru_expiry;
                }
            }
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            buf.pending_write = Some(start..end);
            self.code.push(Asm::WriteBitmapVertices);
        }
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
        self.bitmap_font_positions
            .iter_mut()
            .for_each(|(_, lru)| lru.step());
        self.bitmap_font_transforms
            .iter_mut()
            .for_each(|(_, lru)| lru.step());

        // Remove any fonts which are no longer in use
        self.bitmap_font_positions
            .retain(|(_, data)| !data.lru.is_empty());
        self.bitmap_font_transforms
            .retain(|(_, data)| !data.lru.is_empty());
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            bitmap_font_positions: Default::default(),
            bitmap_font_transforms: Default::default(),
            bitmap_outline_textures: Default::default(),
            bitmap_textures: Default::default(),
            code: Default::default(),
            scalable_font_positions: Default::default(),
            scalable_font_transforms: Default::default(),
        }
    }
}
