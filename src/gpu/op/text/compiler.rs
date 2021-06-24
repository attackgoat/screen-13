use {
    super::{
        bitmap_font::BitmapFont,
        command::Command,
        instruction::{BitmapBindInstruction, Instruction, ScalableBindInstruction},
        scalable_font::ScalableFont,
        Font,
    },
    crate::{
        color::AlphaColor,
        gpu::{
            cache::{Lru, LruCache, Stride},
            data::Mapping,
            op::{DataCopyInstruction, DataTransferInstruction, DataWriteInstruction},
            pool::Pool,
            Texture2d,
        },
        math::{vec3, CoordF, Mat4},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::buffer::Usage as BufferUsage,
    std::{
        borrow::Borrow, cmp::Ordering, marker::PhantomData, ops::Range, ptr::copy_nonoverlapping,
    },
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[derive(Clone, Copy)]
enum Asm {
    BeginBitmapGlyph,
    BeginBitmapOutline,
    BeginScalable,
    BindBitmapGlyph(usize, usize),
    BindBitmapOutline(usize, usize),
    BindScalable(usize),
    CopyBitmapVertices(usize),
    CopyScalableVertices(usize),
    RenderBegin,
    RenderText(u64, u64),
    TransferBitmapData(usize),
    TransferScalableData(usize),
    UpdateBitmapGlyphColor(AlphaColor),
    UpdateBitmapGlyphTransform(Mat4),
    UpdateBitmapOutlineColors(AlphaColor, AlphaColor),
    UpdateBitmapOutlineTransform(Mat4),
    UpdateScalableColor(AlphaColor),
    UpdateScalableTransform(Mat4),
    WriteBitmapVertices(usize),
    WriteScalableVertices(usize),
}

impl Asm {
    fn as_render_text(self) -> Option<(u64, u64)> {
        match self {
            Self::RenderText(start, end) => Some((start, end)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct BitmapChar {
    char: char,
    x: i32,
    y: i32,
}

impl BitmapChar {
    // TODO: Remove?
    /// Each character is rendered as a quad
    const STRIDE: u64 = 96;
}

impl Stride for BitmapChar {
    fn stride(&self) -> u64 {
        Self::STRIDE
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
    fn bind_bitmap(&self, desc_set: usize, idx: usize) -> BitmapBindInstruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_bitmap().unwrap());
        let font_idx = self
            .compiler
            .bitmap_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &self.compiler.bitmap_fonts[font_idx].cache;
        let buf_len = cache.len();

        BitmapBindInstruction {
            buf: &cache.allocation.current,
            buf_len,
            desc_set,
        }
    }

    fn bind_scalable(&self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_scalable().unwrap());
        let font_idx = self
            .compiler
            .scalable_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &self.compiler.scalable_fonts[font_idx].cache;

        Instruction::ScalableBind(ScalableBindInstruction {
            buf: &cache.allocation.current,
            buf_len: cache.len(),
        })
    }

    pub fn bitmap_glyph_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler
            .bitmap_glyph_descriptors
            .iter()
            .map(|font| font.page())
    }

    pub fn bitmap_outline_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler
            .bitmap_outline_descriptors
            .iter()
            .map(|font| font.page())
    }

    fn copy_bitmap_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_bitmap().unwrap());
        let font_idx = self
            .compiler
            .bitmap_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.bitmap_fonts[font_idx].cache;

        Instruction::VertexCopy(DataCopyInstruction {
            buf: &mut cache.allocation.current,
            ranges: cache.pending_copies.as_slice(),
        })
    }

    fn copy_scalable_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_scalable().unwrap());
        let font_idx = self
            .compiler
            .scalable_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.scalable_fonts[font_idx].cache;

        Instruction::VertexCopy(DataCopyInstruction {
            buf: &mut cache.allocation.current,
            ranges: cache.pending_copies.as_slice(),
        })
    }

    /// Returns true if no text is rendered.
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }

    fn transfer_bitmap_data(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(self.cmds[idx].borrow().font().as_bitmap().unwrap());
        let font_idx = self
            .compiler
            .bitmap_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();

        Self::transfer_data(&mut self.compiler.bitmap_fonts[font_idx].cache)
    }

    fn transfer_data<Key>(cache: &mut LruCache<Key, P>) -> Instruction<'_, P> {
        let (src, src_len) = cache.allocation.previous.as_mut().unwrap();

        Instruction::DataTransfer(DataTransferInstruction {
            src,
            src_range: 0..*src_len,
            dst: &mut cache.allocation.current,
        })
    }

    fn transfer_scalable_data(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(self.cmds[idx].borrow().font().as_scalable().unwrap());
        let font_idx = self
            .compiler
            .scalable_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();

        Self::transfer_data(&mut self.compiler.scalable_fonts[font_idx].cache)
    }

    fn write_bitmap_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_bitmap().unwrap());
        let font_idx = self
            .compiler
            .bitmap_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.bitmap_fonts[font_idx].cache;

        Instruction::VertexWrite(DataWriteInstruction {
            buf: &mut cache.allocation.current,
            range: cache.pending_write.as_ref().unwrap().clone(),
        })
    }

    fn write_scalable_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().font().as_scalable().unwrap());
        let font_idx = self
            .compiler
            .scalable_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.scalable_fonts[font_idx].cache;

        Instruction::VertexWrite(DataWriteInstruction {
            buf: &mut cache.allocation.current,
            range: cache.pending_write.as_ref().unwrap().clone(),
        })
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

        Some(match self.compiler.code[idx] {
            Asm::BeginBitmapGlyph => Instruction::BitmapGlyphBegin,
            Asm::BeginBitmapOutline => Instruction::BitmapOutlineBegin,
            Asm::BeginScalable => Instruction::ScalableBegin,
            Asm::BindBitmapGlyph(desc_set, idx) => {
                Instruction::BitmapGlyphBind(self.bind_bitmap(desc_set, idx))
            }
            Asm::BindBitmapOutline(desc_set, idx) => {
                Instruction::BitmapOutlineBind(self.bind_bitmap(desc_set, idx))
            }
            Asm::BindScalable(idx) => self.bind_scalable(idx),
            Asm::CopyBitmapVertices(idx) => self.copy_bitmap_vertices(idx),
            Asm::CopyScalableVertices(idx) => self.copy_scalable_vertices(idx),
            Asm::RenderBegin => Instruction::RenderBegin,
            Asm::RenderText(start, end) => {
                Instruction::RenderText(start as u32 >> 4..end as u32 >> 4)
            }
            Asm::TransferBitmapData(idx) => self.transfer_bitmap_data(idx),
            Asm::TransferScalableData(idx) => self.transfer_scalable_data(idx),
            Asm::UpdateBitmapGlyphColor(glyph_color) => Instruction::BitmapGlyphColor(glyph_color),
            Asm::UpdateBitmapGlyphTransform(view_proj) => {
                Instruction::BitmapGlyphTransform(view_proj)
            }
            Asm::UpdateBitmapOutlineColors(glyph, outline) => {
                Instruction::BitmapOutlineColors(glyph, outline)
            }
            Asm::UpdateBitmapOutlineTransform(view_proj) => {
                Instruction::BitmapOutlineTransform(view_proj)
            }
            Asm::UpdateScalableColor(glyph_color) => Instruction::ScalableColor(glyph_color),
            Asm::UpdateScalableTransform(view_proj) => Instruction::ScalableTransform(view_proj),
            Asm::WriteBitmapVertices(idx) => self.write_bitmap_vertices(idx),
            Asm::WriteScalableVertices(idx) => self.write_scalable_vertices(idx),
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

struct CompiledFont<F, Key, P>
where
    P: 'static + SharedPointerKind,
{
    cache: LruCache<Key, P>,
    font: Shared<F, P>,
}

impl<F, Key, P> CompiledFont<F, Key, P>
where
    P: 'static + SharedPointerKind,
{
    unsafe fn new(pool: &mut Pool<P>, font: &Shared<F, P>) -> Self {
        Self {
            cache: LruCache::new(pool, 4096u64, BufferUsage::VERTEX),
            font: Shared::clone(font),
        }
    }
}

/// Compiles a series of text commands into renderable instructions. The purpose of this structure
/// is two-fold:
/// - Reduce per-text allocations with character vertex caches (they are not cleared after each use)
/// - Store references to the in-use font textures during rendering (this cache is cleared after
///   use)
pub struct Compiler<P>
where
    P: 'static + SharedPointerKind,
{
    bitmap_fonts: Vec<CompiledFont<BitmapFont<P>, BitmapChar, P>>,
    bitmap_glyph_descriptors: Vec<Shared<BitmapFont<P>, P>>,
    bitmap_outline_descriptors: Vec<Shared<BitmapFont<P>, P>>,
    code: Vec<Asm>,
    scalable_fonts: Vec<CompiledFont<ScalableFont, ScalableChar, P>>,
}

impl<P> Compiler<P>
where
    P: 'static + SharedPointerKind,
{
    pub fn compile<'a, 'c, C, T>(
        &'a mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        mut cmds: &'c mut [C],
        dims: CoordF,
    ) -> Compilation<'a, 'c, C, P, T>
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        self.code.push(Asm::RenderBegin);

        // Rearrange the commands so render order doesn't cause unnecessary resource-switching
        Self::sort(&mut cmds);

        // Compile all commands into rendering code
        let mut idx = 0;
        let mut prev_pipeline = None;
        let len = cmds.len();
        while idx < len {
            let cmd = cmds[idx].borrow();
            let pipeline = Self::pipeline(cmd);

            // Switch graphics pipelines as font types change
            if prev_pipeline.is_none() || prev_pipeline.unwrap() != pipeline {
                prev_pipeline = Some(pipeline);
                self.code.push(match pipeline {
                    Pipeline::BitmapGlyph => Asm::BeginBitmapGlyph,
                    Pipeline::BitmapOutline => Asm::BeginBitmapOutline,
                    Pipeline::Scalable => Asm::BeginScalable,
                });
            }

            // Compile all uses of this individual font, advancing `idx` to the following font
            idx = match pipeline {
                Pipeline::BitmapGlyph | Pipeline::BitmapOutline => self.compile_bitmap(
                    #[cfg(feature = "debug-names")]
                    name,
                    pool,
                    cmds,
                    dims,
                    idx,
                ),
                Pipeline::Scalable => self.compile_scalable(
                    #[cfg(feature = "debug-names")]
                    name,
                    pool,
                    cmds,
                    dims,
                    idx,
                ),
            }
        }

        Compilation {
            __: PhantomData,
            cmds,
            compiler: self,
            idx: 0,
        }
    }

    /// Adds `Asm` code which will be used to drive rendering operations. Also maintains a cache
    /// buffer of character and position data which can be drawn efficiently given repeated client
    /// commands.
    fn compile_bitmap<C, T>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmds: &[C],
        dims: CoordF,
        idx: usize,
    ) -> usize
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        let font = cmds[idx].borrow().font();
        let bitmap = font.as_bitmap().unwrap();

        // Figure out how many following commands are the same individual font
        let mut end_idx = idx + 1;
        while end_idx < cmds.len() {
            let next_font = cmds[end_idx].borrow().font();
            if let Some(next_font) = next_font.as_bitmap() {
                if next_font != bitmap {
                    break;
                }

                end_idx += 1;
            } else {
                break;
            }
        }

        // Ensure we've got a compiled font ready
        let font_ptr = Shared::as_ptr(bitmap);
        let font_idx = match self
            .bitmap_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font_ptr))
        {
            Err(idx) => {
                self.bitmap_fonts
                    .insert(idx, unsafe { CompiledFont::new(pool, bitmap) });
                idx
            }
            Ok(idx) => idx,
        };

        // Figure out the total length of all texts using this font
        let text_len: usize = cmds[idx..end_idx]
            .iter()
            .map(|cmd| cmd.borrow().text().len())
            .sum();

        // Allocate enough `buf` to hold everything in the existing compilation and everything we
        // could possibly render for these commands (assuming each character is unique)
        let compilation = &mut self.bitmap_fonts[font_idx];
        let eob = compilation.cache.len();
        let capacity = eob + text_len as u64 * BitmapChar::STRIDE;

        unsafe {
            compilation.cache.realloc(
                #[cfg(feature = "debug-names")]
                &format!("{} bitmap font vertex buffer", name),
                pool,
                capacity,
            );
        }

        // Copy data from the uncompacted end of the GPU buffer back to linear data
        compilation.cache.compact_cache(pool.lru_timestamp);

        // start..end is the back of the buffer where we push new characters
        let start = compilation.cache.len();
        let mut end = start;

        // Bind the texture descriptor and vertex buffer
        let mut prev_pipeline = Self::pipeline(cmds[0].borrow());
        type UpdateTransformFn = fn(Mat4) -> Asm;
        let (bind, mut update_transform) = match prev_pipeline {
            Pipeline::BitmapGlyph => {
                let res = Asm::BindBitmapGlyph(self.bitmap_glyph_descriptors.len(), idx);
                self.bitmap_glyph_descriptors.push(Shared::clone(bitmap));
                let update_transform = |view_proj| Asm::UpdateBitmapGlyphTransform(view_proj);
                (res, update_transform as UpdateTransformFn)
            }
            Pipeline::BitmapOutline => {
                let res = Asm::BindBitmapOutline(self.bitmap_outline_descriptors.len(), idx);
                self.bitmap_outline_descriptors.push(Shared::clone(bitmap));
                let update_transform = |view_proj| Asm::UpdateBitmapOutlineTransform(view_proj);
                (res, update_transform as UpdateTransformFn)
            }
            _ => unreachable!(),
        };
        self.code.push(bind);

        // Fill the vertex buffer for all commands which use this font
        for (idx, cmd) in cmds[idx..end_idx].iter().enumerate() {
            let cmd = cmd.borrow();

            // Change pipeline and re-bind if needed (this will only switch from glyph to outline!!)
            let pipeline = Self::pipeline(cmd);
            if pipeline != prev_pipeline {
                prev_pipeline = pipeline;
                self.code.push(Asm::BeginBitmapOutline);
                self.code.push(Asm::BindBitmapOutline(
                    self.bitmap_outline_descriptors.len(),
                    idx,
                ));
                self.bitmap_outline_descriptors.push(Shared::clone(bitmap));
                update_transform = |view_proj| Asm::UpdateBitmapOutlineTransform(view_proj);
            }

            // Always update transform
            let view_proj = if let Some(view_proj) = cmd.as_transform() {
                view_proj
            } else {
                // PERF: Should hand roll this
                // Read as:
                // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
                // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
                let layout = cmd.as_position().unwrap();
                Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
                    * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
                    * Mat4::from_translation(vec3(layout.x / dims.x, layout.y / dims.y, 0.0))
            };
            self.code.push(update_transform(view_proj));

            // Always update color(s)
            let glyph_color = cmd.glyph_color();
            match pipeline {
                Pipeline::BitmapGlyph => self.code.push(Asm::UpdateBitmapGlyphColor(glyph_color)),
                Pipeline::BitmapOutline => self.code.push(Asm::UpdateBitmapOutlineColors(
                    glyph_color,
                    cmd.outline_color().unwrap(),
                )),
                _ => unreachable!(),
            }

            // We are going to submit rendering commands but we need to keep track of the current
            // asm code index so that we can ensure the 'copy to gpu' asm code is executed before
            // rendering
            let code_idx_before_text = self.code.len();

            // Characters will generally follow each other so we keep a running range of renderable
            // text in order to reduce the need to sort/re-group later. This requires a fix-up step
            // after the loop to capture the last range!
            let mut text_range: Option<Range<u64>> = None;

            // Make sure all characters are in the lru data
            for char in bitmap.parse(cmd.text()) {
                let key = BitmapChar {
                    char: char.char(),
                    x: char.screen_rect.x,
                    y: char.screen_rect.y,
                };
                let offset = match compilation
                    .cache
                    .items
                    .binary_search_by(|probe| probe.key.cmp(&key))
                {
                    Err(idx) => {
                        // Cache the vertices for this character
                        let vertices = bitmap.tessellate(&char);
                        let start = end;
                        end += vertices.len() as u64;

                        unsafe {
                            let mut mapped_range = compilation
                                .cache
                                .allocation
                                .current
                                .map_range_mut(start..end)
                                .unwrap();
                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                vertices.len() as _,
                            );

                            Mapping::flush(&mut mapped_range).unwrap();
                        }

                        // Create a new cache entry for this character
                        compilation.cache.usage.push((start, key));
                        compilation.cache.items.insert(
                            idx,
                            Lru {
                                expiry: pool.lru_expiry,
                                key,
                                offset: start,
                            },
                        );
                        start
                    }
                    Ok(idx) => {
                        let lru = &mut compilation.cache.items[idx];
                        lru.expiry = pool.lru_expiry;
                        lru.offset
                    }
                };

                // Handle text rendering
                if let Some(range) = &mut text_range {
                    if range.end == offset {
                        // Contiguous: Extend current text range with this character
                        range.end += BitmapChar::STRIDE;
                    } else {
                        // Non-contiguous: Render the current text range and start a new one
                        self.code.push(Asm::RenderText(range.start, range.end));
                        text_range = Some(offset..offset + BitmapChar::STRIDE);
                    }
                } else {
                    // First text range
                    text_range = Some(offset..offset + BitmapChar::STRIDE);
                }
            }

            // Fix-up step: Commit the last text range, if any
            if let Some(range) = text_range {
                self.code.push(Asm::RenderText(range.start, range.end));
            }

            // The rendered text may have been found in non-contiguous sections of the data - so we sort
            // them and reduce rendering commands by joining any groups the sorting has formed
            self.code[code_idx_before_text..].sort_unstable_by(|lhs, rhs| {
                lhs.as_render_text()
                    .unwrap()
                    .0
                    .cmp(&rhs.as_render_text().unwrap().0)
            });
            let mut read_idx = code_idx_before_text + 1;
            let mut write_idx = code_idx_before_text;
            while read_idx < self.code.len() {
                let (read_start, read_end) = self.code[read_idx].as_render_text().unwrap();
                let (write_start, write_end) = self.code[write_idx].as_render_text().unwrap();
                if read_start == write_end {
                    self.code[write_idx] = Asm::RenderText(write_start, read_end);
                    read_idx += 1;
                } else {
                    read_idx += 1;
                    write_idx += 1;
                }
            }

            // Trim off any excess rendering commands
            self.code.truncate(write_idx + 1);
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            compilation.cache.pending_write = Some(start..end);
            self.code.insert(0, Asm::WriteBitmapVertices(idx));
        }

        // Handle copied ranges from earlier
        if !compilation.cache.pending_copies.is_empty() {
            self.code.insert(0, Asm::CopyBitmapVertices(idx));
        }

        // Transfer data from the previous GPU buffer to the new one, if we have a previous buffer
        if compilation.cache.allocation.previous.is_some() {
            self.code.insert(0, Asm::TransferBitmapData(idx));
        }

        end_idx
    }

    fn compile_scalable<C, T>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmds: &[C],
        dims: CoordF,
        idx: usize,
    ) -> usize
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        let font = cmds[idx].borrow().font();
        let scalable = font.as_scalable().unwrap();

        // Figure out how many following commands are the same individual font
        let mut end_idx = idx + 1;
        while end_idx < cmds.len() {
            let next_font = cmds[end_idx].borrow().font();
            if let Some(next_font) = next_font.as_scalable() {
                if next_font != scalable {
                    break;
                }

                end_idx += 1;
            } else {
                // This path won't happen because we sorted the commands bitmapped -> scalable
                unreachable!();
                //break;
            }
        }

        // Ensure we've got a compiled font ready
        let font_ptr = Shared::as_ptr(scalable);
        let font_idx = match self
            .scalable_fonts
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font_ptr))
        {
            Err(idx) => {
                self.scalable_fonts
                    .insert(idx, unsafe { CompiledFont::new(pool, scalable) });
                idx
            }
            Ok(idx) => idx,
        };

        /*/ Figure out the total length of all texts using this font
        let text_len: usize = cmds[idx..end_idx]
            .iter()
            .map(|cmd| cmd.borrow().text().len())
            .sum();

        // Allocate enough `buf` to hold everything in the existing compilation and everything we
        // could possibly render for these commands (assuming each character is unique)
        let compilation = &mut self.scalable_fonts[font_idx];
        let eob = compilation.cache.len();
        let capacity = eob + text_len as u64 * BitmapChar::STRIDE;

        unsafe {
            compilation.cache.realloc(
                #[cfg(feature = "debug-names")]
                &format!("{} scalable font vertex buffer", name),
                pool,
                capacity,
            );
        }

        // Copy data from the uncompacted end of the GPU buffer back to linear data
        compilation.cache.compact_cache(pool.lru_timestamp);

        // start..end is the back of the buffer where we push new characters
        let start = compilation.cache.len();
        let mut end = start;

        // Bind the vertex buffer
        self.code.push(Asm::BindScalable(idx));

        // Fill the vertex buffer for all commands which use this font
        for (idx, cmd) in cmds[idx..end_idx].iter().enumerate() {
            let cmd = cmd.borrow();

            // Always update transform
            let view_proj = if let Some(view_proj) = cmd.as_transform() {
                view_proj
            } else {
                // PERF: Should hand roll this
                // Read as:
                // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
                // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
                let layout = cmd.as_position().unwrap();
                Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
                    * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
                    * Mat4::from_translation(vec3(layout.x / dims.x, layout.y / dims.y, 0.0))
            };
            self.code.push(Asm::UpdateScalableTransform(view_proj));

            // Always update color(s)
            self.code.push(Asm::UpdateScalableColor(cmd.glyph_color()));

            // We are going to submit rendering commands but we need to keep track of the current
            // asm code index so that we can ensure the 'copy to gpu' asm code is executed before
            // rendering
            let code_idx_before_text = self.code.len();

            // Characters will generally follow each other so we keep a running range of renderable
            // text in order to reduce the need to sort/re-group later. This requires a fix-up step
            // after the loop to capture the last range!
            let mut text_range: Option<Range<u64>> = None;

            // Make sure all characters are in the lru data
            for char in bitmap.parse(cmd.text()) {
                let key = BitmapChar {
                    char: char.char(),
                    x: char.screen_rect.x,
                    y: char.screen_rect.y,
                };
                let offset = match compilation
                    .cache
                    .items
                    .binary_search_by(|probe| probe.key.cmp(&key))
                {
                    Err(idx) => {
                        // Cache the vertices for this character
                        let vertices = bitmap.tessellate(&char);
                        let start = end;
                        end += vertices.len() as u64;

                        unsafe {
                            let mut mapped_range = compilation
                                .cache
                                .allocation
                                .current
                                .map_range_mut(start..end)
                                .unwrap();
                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                vertices.len() as _,
                            );

                            Mapping::flush(&mut mapped_range).unwrap();
                        }

                        // Create a new cache entry for this character
                        compilation.cache.usage.push((start, key));
                        compilation.cache.items.insert(
                            idx,
                            Lru {
                                expiry: pool.lru_expiry,
                                key,
                                offset: start,
                            },
                        );
                        start
                    }
                    Ok(idx) => {
                        let lru = &mut compilation.cache.items[idx];
                        lru.expiry = pool.lru_expiry;
                        lru.offset
                    }
                };

                // Handle text rendering
                if let Some(range) = &mut text_range {
                    if range.end == offset {
                        // Contiguous: Extend current text range with this character
                        range.end += BitmapChar::STRIDE;
                    } else {
                        // Non-contiguous: Render the current text range and start a new one
                        self.code.push(Asm::RenderText(range.start, range.end));
                        text_range = Some(offset..offset + BitmapChar::STRIDE);
                    }
                } else {
                    // First text range
                    text_range = Some(offset..offset + BitmapChar::STRIDE);
                }
            }

            // Fix-up step: Commit the last text range, if any
            if let Some(range) = text_range {
                self.code.push(Asm::RenderText(range.start, range.end));
            }

            // The rendered text may have been found in non-contiguous sections of the data - so we sort
            // them and reduce rendering commands by joining any groups the sorting has formed
            self.code[code_idx_before_text..].sort_unstable_by(|lhs, rhs| {
                lhs.as_render_text()
                    .unwrap()
                    .0
                    .cmp(&rhs.as_render_text().unwrap().0)
            });
            let mut read_idx = code_idx_before_text + 1;
            let mut write_idx = code_idx_before_text;
            while read_idx < self.code.len() {
                let (read_start, read_end) = self.code[read_idx].as_render_text().unwrap();
                let (write_start, write_end) = self.code[write_idx].as_render_text().unwrap();
                if read_start == write_end {
                    self.code[write_idx] = Asm::RenderText(write_start, read_end);
                    read_idx += 1;
                } else {
                    read_idx += 1;
                    write_idx += 1;
                }
            }

            // Trim off any excess rendering commands
            self.code.truncate(write_idx + 1);
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            compilation.cache.pending_write = Some(start..end);
            self.code.insert(0, Asm::WriteBitmapVertices(idx));
        }

        // Handle copied ranges from earlier
        if !compilation.cache.pending_copies.is_empty() {
            self.code.insert(0, Asm::CopyBitmapVertices(idx));
        }

        // Transfer data from the previous GPU buffer to the new one, if we have a previous buffer
        if compilation.cache.allocation.previous.is_some() {
            self.code.insert(0, Asm::TransferBitmapData(idx));
        }*/

        end_idx
    }

    fn pipeline<T>(cmd: &Command<P, T>) -> Pipeline
    where
        T: AsRef<str>,
    {
        match cmd {
            Command::Position(cmd) => {
                if cmd.outline_color.is_none() {
                    Pipeline::BitmapGlyph
                } else {
                    Pipeline::BitmapOutline
                }
            }
            Command::Transform(cmd) => {
                if cmd.outline_color.is_none() {
                    Pipeline::BitmapGlyph
                } else {
                    Pipeline::BitmapOutline
                }
            }
            _ => Pipeline::Scalable,
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile`
    /// function.
    ///
    /// Must NOT be called before the previously drawn frame is completed.
    pub(super) fn reset(&mut self) {
        self.bitmap_glyph_descriptors.clear();
        self.bitmap_outline_descriptors.clear();

        // TODO: Can these things be just two functions called three times each?

        // Advance the least-recently-used caching algorithm one step forward
        self.bitmap_fonts
            .iter_mut()
            .for_each(|compilation| compilation.cache.reset());
        self.scalable_fonts
            .iter_mut()
            .for_each(|compilation| compilation.cache.reset());

        // Remove any fonts which are no longer in use
        self.bitmap_fonts
            .retain(|compilation| !compilation.cache.items.is_empty());
        self.scalable_fonts
            .retain(|compilation| !compilation.cache.items.is_empty());
    }

    /// Sorts commands into a predictable and efficient order for drawing.
    fn sort<C, T>(cmds: &mut [C])
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        // Unstable because we don't claim to offering any ordering within a single batch
        cmds.sort_unstable_by(|lhs, rhs| {
            let lhs = lhs.borrow();
            let rhs = rhs.borrow();

            // The output order should be:
            // 1. Bitmapped fonts (sorted by pointer)
            // 3. Scalable fonts (sorted by pointer)

            match lhs.font() {
                Font::Bitmap(lhs_font) => match rhs.font() {
                    Font::Bitmap(rhs_font) => {
                        match Shared::as_ptr(&lhs_font).cmp(&Shared::as_ptr(&rhs_font)) {
                            Ordering::Equal => lhs.outline_color().cmp(&rhs.outline_color()),
                            ne => ne,
                        }
                    }
                    _ => Ordering::Less,
                },
                Font::Scalable(lhs_font) => match rhs.font() {
                    Font::Scalable(rhs_font) => {
                        Shared::as_ptr(&lhs_font).cmp(&Shared::as_ptr(&rhs_font))
                    }
                    _ => Ordering::Greater,
                },
            }
        });
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            bitmap_fonts: Default::default(),
            bitmap_glyph_descriptors: Default::default(),
            bitmap_outline_descriptors: Default::default(),
            code: Default::default(),
            scalable_fonts: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Pipeline {
    BitmapGlyph,
    BitmapOutline,
    Scalable,
}

#[derive(Clone, Copy, PartialEq)]
struct ScalableChar {
    char: char,
    stride: u32,
    x: f32,
    y: f32,
}

impl Eq for ScalableChar {}

impl Ord for ScalableChar {
    fn cmp(&self, other: &Self) -> Ordering {
        let res = self.char.cmp(&other.char);
        if res != Ordering::Less {
            return res;
        }

        // TODO: Should probably also store and compare SIZE and just not compare this field? What about eq and partial eq and partial ord!!
        let res = self.stride.cmp(&other.stride);
        if res != Ordering::Less {
            return res;
        }

        let res = self.x.partial_cmp(&other.x).unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        self.y.partial_cmp(&other.y).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for ScalableChar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Stride for ScalableChar {
    fn stride(&self) -> u64 {
        //self.stride as _
        todo!()
    }
}
