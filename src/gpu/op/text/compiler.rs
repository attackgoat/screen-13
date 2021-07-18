use {
    super::{
        bitmap_font::BitmapFont, dyn_atlas::DynamicAtlas, glyph::Glyph as _,
        vector_font::VectorFont, Command, Instruction, VertexBindInstruction,
    },
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
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
    BeginBitmap,
    BeginText,
    BeginVector,
    BindBitmapVertices(usize),
    BindBitmapDescriptorSet(usize),
    BindVectorDescriptorSet(usize),
    BindVectorVertices(usize),
    CopyBitmapVertices(usize),
    CopyVectorGlyphs(usize),
    CopyVectorVertices(usize),
    RenderText(usize, u64, u64),
    TransferBitmapData(usize),
    TransferVectorData(usize),
    UpdateBitmapColors(AlphaColor, AlphaColor),
    UpdateBitmapTransform(Mat4),
    UpdateVectorColor(AlphaColor),
    UpdateVectorTransform(Mat4),
    WriteBitmapVertices(usize),
    WriteVectorVertices(usize),
}

impl Asm {
    fn as_render_text(self) -> Option<(usize, u64, u64)> {
        match self {
            Self::RenderText(desc_set, start, end) => Some((desc_set, start, end)),
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
    fn bind_bitmap_vertices(&self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().bitmap_font().unwrap());
        let font_idx = self
            .compiler
            .bitmap_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &self.compiler.bitmap_chars[font_idx].cache;
        let buf_len = cache.len();

        Instruction::VertexBind(VertexBindInstruction {
            buf: &cache.allocation.current,
            buf_len,
        })
    }

    fn bind_vector_vertices(&self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().vector_font().unwrap());
        let font_idx = self
            .compiler
            .vector_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &self.compiler.vector_chars[font_idx].cache;

        Instruction::VertexBind(VertexBindInstruction {
            buf: &cache.allocation.current,
            buf_len: cache.len(),
        })
    }

    pub fn bitmap_desc_sets(&self) -> usize {
        self.compiler.bitmap_desc_sets
    }

    pub fn bitmap_textures(&self) -> impl Iterator<Item = &Texture2d> {
        self.compiler
            .bitmap_fonts
            .iter()
            .flat_map(|font| font.pages())
    }

    fn copy_bitmap_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().bitmap_font().unwrap());
        let font_idx = self
            .compiler
            .bitmap_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.bitmap_chars[font_idx].cache;

        Instruction::VertexCopy(DataCopyInstruction {
            buf: &mut cache.allocation.current,
            ranges: cache.pending_copies.as_slice(),
        })
    }

    fn copy_vector_glyphs(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().vector_font().unwrap());
        let atlas_idx = self
            .compiler
            .vector_atlas
            .binary_search_by(|probe| Shared::as_ptr(probe.font()).cmp(&font))
            .unwrap();
        let atlas = &mut self.compiler.vector_atlas[atlas_idx];

        Instruction::VectorGlyphCopy(atlas)
    }

    fn copy_vector_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().vector_font().unwrap());
        let font_idx = self
            .compiler
            .vector_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.vector_chars[font_idx].cache;

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
        let font = Shared::as_ptr(self.cmds[idx].borrow().bitmap_font().unwrap());
        let font_idx = self
            .compiler
            .bitmap_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();

        Self::transfer_data(&mut self.compiler.bitmap_chars[font_idx].cache)
    }

    fn transfer_data<Key>(cache: &mut LruCache<Key, P>) -> Instruction<'_, P> {
        let (src, src_len) = cache.allocation.previous.as_mut().unwrap();

        Instruction::DataTransfer(DataTransferInstruction {
            src,
            src_range: 0..*src_len,
            dst: &mut cache.allocation.current,
        })
    }

    fn transfer_vector_data(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(self.cmds[idx].borrow().vector_font().unwrap());
        let font_idx = self
            .compiler
            .vector_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();

        Self::transfer_data(&mut self.compiler.vector_chars[font_idx].cache)
    }

    pub fn vector_desc_sets(&self) -> usize {
        self.compiler.vector_desc_sets
    }

    pub fn vector_textures(&self) -> impl Iterator<Item = &Texture2d> {
        self.compiler
            .vector_atlas
            .iter()
            .flat_map(|atlas| atlas.pages())
    }

    fn write_bitmap_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().bitmap_font().unwrap());
        let font_idx = self
            .compiler
            .bitmap_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.bitmap_chars[font_idx].cache;

        Instruction::VertexWrite(DataWriteInstruction {
            buf: &mut cache.allocation.current,
            range: cache.pending_write.as_ref().unwrap().clone(),
        })
    }

    fn write_vector_vertices(&mut self, idx: usize) -> Instruction<'_, P> {
        let font = Shared::as_ptr(&self.cmds[idx].borrow().vector_font().unwrap());
        let font_idx = self
            .compiler
            .vector_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font))
            .unwrap();
        let cache = &mut self.compiler.vector_chars[font_idx].cache;

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
            Asm::BeginBitmap => Instruction::BitmapBegin,
            Asm::BeginText => Instruction::TextBegin,
            Asm::BeginVector => Instruction::VectorBegin,
            Asm::BindBitmapDescriptorSet(idx) => Instruction::BitmapBindDescriptorSet(idx),
            Asm::BindBitmapVertices(idx) => self.bind_bitmap_vertices(idx),
            Asm::BindVectorDescriptorSet(idx) => Instruction::VectorBindDescriptorSet(idx),
            Asm::BindVectorVertices(idx) => self.bind_vector_vertices(idx),
            Asm::CopyBitmapVertices(idx) => self.copy_bitmap_vertices(idx),
            Asm::CopyVectorGlyphs(idx) => self.copy_vector_glyphs(idx),
            Asm::CopyVectorVertices(idx) => self.copy_vector_vertices(idx),
            Asm::RenderText(_, start, end) => {
                Instruction::TextRender(start as u32 >> 4..end as u32 >> 4)
            }
            Asm::TransferBitmapData(idx) => self.transfer_bitmap_data(idx),
            Asm::TransferVectorData(idx) => self.transfer_vector_data(idx),
            Asm::UpdateBitmapColors(glyph, outline) => Instruction::BitmapColors(glyph, outline),
            Asm::UpdateBitmapTransform(view_proj) => Instruction::BitmapTransform(view_proj),
            Asm::UpdateVectorColor(glyph) => Instruction::VectorColor(glyph),
            Asm::UpdateVectorTransform(view_proj) => Instruction::VectorTransform(view_proj),
            Asm::WriteBitmapVertices(idx) => self.write_bitmap_vertices(idx),
            Asm::WriteVectorVertices(idx) => self.write_vector_vertices(idx),
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

/// Holds a cache of character vertices for a given font
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
            cache: LruCache::new(pool, 4096, BufferUsage::VERTEX),
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
    bitmap_chars: Vec<CompiledFont<BitmapFont<P>, BitmapChar, P>>,
    bitmap_desc_sets: usize,
    bitmap_fonts: Vec<Shared<BitmapFont<P>, P>>,
    code: Vec<Asm>,
    vector_atlas: Vec<DynamicAtlas<P>>,
    vector_chars: Vec<CompiledFont<VectorFont, VectorChar, P>>,
    vector_desc_sets: usize,
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
        atlas_buf_len: u64,
        atlas_dims: u32,
    ) -> Compilation<'a, 'c, C, P, T>
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        self.code.push(Asm::BeginText);

        // Rearrange the commands so render order doesn't cause unnecessary resource-switching
        Self::sort_cmds(&mut cmds);

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
                    Pipeline::Bitmap => Asm::BeginBitmap,
                    Pipeline::Vector => Asm::BeginVector,
                });
            }

            // Compile all uses of this individual font, advancing `idx` to the following font
            idx = match pipeline {
                Pipeline::Bitmap => self.compile_bitmap(
                    #[cfg(feature = "debug-names")]
                    name,
                    pool,
                    cmds,
                    dims,
                    idx,
                ),
                Pipeline::Vector => self.compile_vector(
                    #[cfg(feature = "debug-names")]
                    name,
                    pool,
                    cmds,
                    dims,
                    idx,
                    atlas_buf_len,
                    atlas_dims,
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
        let font_ref = cmds[idx].borrow().bitmap_font();
        let font = font_ref.unwrap();

        // Figure out how many following commands are the same individual font
        let mut end_idx = idx + 1;
        while end_idx < cmds.len() {
            if let Some(next_font) = cmds[end_idx].borrow().bitmap_font() {
                if font != next_font {
                    break;
                }

                end_idx += 1;
            } else {
                break;
            }
        }

        // Ensure we've got a compiled font ready
        let font_ptr = Shared::as_ptr(font);
        let font_idx = match self
            .bitmap_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font_ptr))
        {
            Err(idx) => {
                self.bitmap_chars
                    .insert(idx, unsafe { CompiledFont::new(pool, font) });
                idx
            }
            Ok(idx) => idx,
        };

        // Store a reference to the font so we can later bind these textures
        let desc_set_base = self.bitmap_desc_sets; //Self::bitmap_desc_sets(self.bitmap_fonts.iter());
        self.bitmap_fonts.push(Shared::clone(font));
        self.bitmap_desc_sets += font.pages().len();

        // Figure out the total length of all texts using this font
        let text_len: usize = cmds[idx..end_idx]
            .iter()
            .map(|cmd| cmd.borrow().text().len())
            .sum();

        // Allocate enough `buf` to hold everything in the existing chars and everything we
        // could possibly render for these commands (assuming each character is unique)
        let chars = &mut self.bitmap_chars[font_idx];
        let cache_len = chars.cache.len();
        let capacity = cache_len + text_len as u64 * BitmapChar::STRIDE;

        unsafe {
            chars.cache.realloc(
                #[cfg(feature = "debug-names")]
                &format!("{} bitmap font vertex buffer", name),
                pool,
                capacity,
            );
        }

        // Copy data from the uncompacted end of the GPU buffer back to linear data
        chars.cache.compact_usage(pool.lru_timestamp);

        // start..end is the back of the buffer where we push new characters
        let start = chars.cache.len();
        let mut end = start;

        // Bind the vertex buffer
        self.code.push(Asm::BindBitmapVertices(idx));

        // Fill the vertex buffer for all commands which use this font
        for cmd in cmds[idx..end_idx].iter() {
            let cmd = cmd.borrow();

            // Always update transform
            let view_proj = if let Some(view_proj) = cmd.transform() {
                view_proj
            } else {
                // PERF: Should hand roll this
                // Read as:
                // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
                // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
                let layout = cmd.position().unwrap();
                Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
                    * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
                    * Mat4::from_translation(vec3(layout.x / dims.x, layout.y / dims.y, 0.0))
            };
            self.code.push(Asm::UpdateBitmapTransform(view_proj));

            // Always update color(s)
            self.code.push(Asm::UpdateBitmapColors(
                cmd.glyph_color(),
                cmd.outline_color().unwrap_or(TRANSPARENT_BLACK),
            ));

            // We are going to submit rendering commands but we need to keep track of the current
            // asm code index so that we can ensure the 'copy to gpu' asm code is executed before
            // rendering
            let code_idx_before_text = self.code.len();

            // Characters will generally follow each other so we keep a running range of renderable
            // text in order to reduce the need to sort/re-group later. This requires a fix-up step
            // after the loop to capture the last range! First value is which descriptor set index.
            let mut text: Option<(usize, Range<u64>)> = None;

            // Make sure all characters are in the lru data
            for glyph in font.parse(cmd.text()) {
                let key = BitmapChar {
                    char: glyph.char(),
                    x: glyph.screen_rect.x,
                    y: glyph.screen_rect.y,
                };
                let page_idx = glyph.page_index;
                // TODO: Before searching we should check the index which follows the previous one
                let offset = match chars
                    .cache
                    .items
                    .binary_search_by(|probe| probe.key.cmp(&key))
                {
                    Err(idx) => {
                        // Cache the vertices for this character
                        let vertices = glyph.tessellate();
                        let offset = end;
                        end += vertices.len() as u64;

                        unsafe {
                            let mut mapped_range = chars
                                .cache
                                .allocation
                                .current
                                .map_range_mut(offset..end)
                                .unwrap();
                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                vertices.len() as _,
                            );

                            Mapping::flush(&mut mapped_range).unwrap();
                        }

                        // Create a new cache entry for this character
                        chars.cache.usage.push((offset, key));
                        chars.cache.items.insert(
                            idx,
                            Lru {
                                expiry: pool.lru_expiry,
                                key,
                                offset,
                            },
                        );
                        offset
                    }
                    Ok(idx) => {
                        let lru = &mut chars.cache.items[idx];
                        lru.expiry = pool.lru_expiry;
                        lru.offset
                    }
                };

                // Handle text rendering
                let desc_set = desc_set_base + page_idx as usize;
                if let Some((idx, range)) = &mut text {
                    if desc_set == *idx && range.end == offset {
                        // Contiguous: Extend current text with this character
                        range.end += BitmapChar::STRIDE;
                    } else {
                        // Non-contiguous: Render the current text and start new text
                        self.code
                            .push(Asm::RenderText(desc_set, range.start, range.end));
                        text = Some((desc_set, offset..offset + BitmapChar::STRIDE));
                    }
                } else {
                    // First text
                    text = Some((desc_set, offset..offset + BitmapChar::STRIDE));
                }
            }

            // Fix-up step: Commit the last text, if any
            if let Some((desc_set, range)) = text {
                self.code
                    .push(Asm::RenderText(desc_set, range.start, range.end));
            } else {
                continue;
            }

            // The rendered text may have been found in non-contiguous sections of the data - so we
            // sort them and reduce rendering commands by joining any groups the sorting has formed
            // We will also jam in some bind-descriptor-set code
            Self::sort_code(&mut self.code[code_idx_before_text..]);
            let mut desc_set = self.code[code_idx_before_text].as_render_text().unwrap().0;
            let mut read_idx = code_idx_before_text + 2;
            let mut write_idx = code_idx_before_text + 1;
            self.code
                .insert(code_idx_before_text, Asm::BindBitmapDescriptorSet(desc_set));
            while read_idx < self.code.len() {
                let (read_desc_set, read_start, read_end) =
                    self.code[read_idx].as_render_text().unwrap();
                let (write_desc_set, write_start, write_end) =
                    self.code[write_idx].as_render_text().unwrap();
                if read_desc_set != desc_set {
                    self.code
                        .insert(read_idx, Asm::BindBitmapDescriptorSet(desc_set));
                    desc_set = read_desc_set;
                    read_idx += 1;
                    write_idx += 1;
                }

                if read_desc_set == write_desc_set && read_start == write_end {
                    self.code[write_idx] = Asm::RenderText(read_desc_set, write_start, read_end);
                } else {
                    write_idx += 1;
                }

                read_idx += 1;
            }

            // Trim off any excess rendering commands
            self.code.truncate(write_idx + 1);
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            chars.cache.pending_write = Some(start..end);
            self.code.insert(0, Asm::WriteBitmapVertices(idx));
        }

        // Handle copied ranges from earlier
        if !chars.cache.pending_copies.is_empty() {
            self.code.insert(0, Asm::CopyBitmapVertices(idx));
        }

        // Transfer data from the previous GPU buffer to the new one, if we have a previous buffer
        if chars.cache.allocation.previous.is_some() {
            self.code.insert(0, Asm::TransferBitmapData(idx));
        }

        end_idx
    }

    fn compile_vector<C, T>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        cmds: &[C],
        dims: CoordF,
        idx: usize,
        atlas_buf_len: u64,
        atlas_dims: u32,
    ) -> usize
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        let font = cmds[idx].borrow().vector_font().unwrap();

        // Figure out how many following commands are the same individual font
        let mut end_idx = idx + 1;
        while end_idx < cmds.len() {
            if font != cmds[end_idx].borrow().vector_font().unwrap() {
                break;
            }

            end_idx += 1;
        }

        // Ensure we've got a compiled font ready
        let font_ptr = Shared::as_ptr(font);
        let font_idx = match self
            .vector_chars
            .binary_search_by(|probe| Shared::as_ptr(&probe.font).cmp(&font_ptr))
        {
            Err(idx) => {
                self.vector_chars
                    .insert(idx, unsafe { CompiledFont::new(pool, font) });
                idx
            }
            Ok(idx) => idx,
        };

        // Store a references to the font so we can later bind these textures
        let desc_set_base = self.vector_desc_sets;
        let atlas_idx = match self
            .vector_atlas
            .binary_search_by(|probe| Shared::as_ptr(&probe.font()).cmp(&font_ptr))
        {
            Err(idx) => {
                self.vector_atlas.insert(idx, DynamicAtlas::new(font));
                idx
            }
            Ok(idx) => idx,
        };
        let atlas = &mut self.vector_atlas[atlas_idx];

        // Figure out the total length of all texts using this font
        let text_len: usize = cmds[idx..end_idx]
            .iter()
            .map(|cmd| cmd.borrow().text().len())
            .sum();

        // Allocate enough `buf` to hold everything in the existing chars and everything we
        // could possibly render for these commands (assuming each character is unique)
        let chars = &mut self.vector_chars[font_idx];
        let cache_len = chars.cache.len();
        let capacity = cache_len + text_len as u64 * VectorChar::STRIDE;

        unsafe {
            chars.cache.realloc(
                #[cfg(feature = "debug-names")]
                &format!("{} vector font vertex buffer", name),
                pool,
                capacity,
            );
        }

        // Copy data from the uncompacted end of the GPU buffer back to linear data
        chars.cache.compact_usage(pool.lru_timestamp);

        // start..end is the back of the buffer where we push new characters
        let start = chars.cache.len();
        let mut end = start;

        // Bind the vertex buffer
        self.code.push(Asm::BindVectorVertices(idx));

        // Fill the vertex buffer for all commands which use this font
        for cmd in cmds[idx..end_idx].iter() {
            let cmd = cmd.borrow();

            // Quantize incoming size to 1/3 of a pixel and skip tiny text
            let size = (cmd.size() * 3.0).trunc() / 3.0;
            if size < 1.0 / 3.0 {
                continue;
            }

            // Always update transform
            let view_proj = if let Some(view_proj) = cmd.transform() {
                view_proj
            } else {
                // PERF: Should hand roll this
                // Read as:
                // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
                // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
                let layout = cmd.position().unwrap();
                Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
                    * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
                    * Mat4::from_translation(vec3(layout.x / dims.x, layout.y / dims.y, 0.0))
            };
            self.code.push(Asm::UpdateVectorTransform(view_proj));

            // Always update color
            self.code.push(Asm::UpdateVectorColor(cmd.glyph_color()));

            // We are going to submit rendering commands but we need to keep track of the current
            // asm code index so that we can ensure the 'copy to gpu' asm code is executed before
            // rendering
            let code_idx_before_text = self.code.len();

            // Characters will generally follow each other so we keep a running range of renderable
            // text in order to reduce the need to sort/re-group later. This requires a fix-up step
            // after the loop to capture the last range! First value is which descriptor set index.
            let mut text: Option<(usize, Range<u64>)> = None;

            // Make sure all characters are in the lru data
            let lru_expiry = pool.lru_expiry;
            for (char, glyph) in atlas.parse(pool, atlas_buf_len, atlas_dims, size, cmd.text()) {
                let key = VectorChar {
                    char,
                    size: size.to_bits(),
                    x: glyph.screen_rect.pos.x.to_bits(),
                    y: glyph.screen_rect.pos.y.to_bits(),
                };
                let page_idx = glyph.page_idx;
                let offset = match chars
                    .cache
                    .items
                    .binary_search_by(|probe| probe.key.cmp(&key))
                {
                    Err(idx) => {
                        // Cache the vertices for this character
                        let vertices = glyph.tessellate();
                        let offset = end;
                        end += vertices.len() as u64;

                        unsafe {
                            let mut mapped_range = chars
                                .cache
                                .allocation
                                .current
                                .map_range_mut(offset..end)
                                .unwrap();
                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                vertices.len() as _,
                            );

                            Mapping::flush(&mut mapped_range).unwrap();
                        }

                        // Create a new cache entry for this character
                        chars.cache.usage.push((offset, key));
                        chars.cache.items.insert(
                            idx,
                            Lru {
                                expiry: lru_expiry,
                                key,
                                offset,
                            },
                        );
                        offset
                    }
                    Ok(idx) => {
                        let lru = &mut chars.cache.items[idx];
                        lru.expiry = lru_expiry;
                        lru.offset
                    }
                };

                // Handle text rendering
                let desc_set = desc_set_base + page_idx;
                if let Some((idx, range)) = &mut text {
                    if desc_set == *idx && range.end == offset {
                        // Contiguous: Extend current text with this character
                        range.end += VectorChar::STRIDE;
                    } else {
                        // Non-contiguous: Render the current text and start new text
                        self.code
                            .push(Asm::RenderText(desc_set, range.start, range.end));
                        text = Some((desc_set, offset..offset + VectorChar::STRIDE));
                    }
                } else {
                    // First text
                    text = Some((desc_set, offset..offset + VectorChar::STRIDE));
                }
            }

            // Fix-up step: Commit the last text range, if any
            if let Some((desc_set, range)) = text {
                self.code
                    .push(Asm::RenderText(desc_set, range.start, range.end));
            } else {
                continue;
            }

            // The rendered text may have been found in non-contiguous sections of the data - so we sort
            // them and reduce rendering commands by joining any groups the sorting has formed
            // We will also jam in some bind-descriptor-set code
            Self::sort_code(&mut self.code[code_idx_before_text..]);
            let mut desc_set = self.code[code_idx_before_text].as_render_text().unwrap().0;
            let mut read_idx = code_idx_before_text + 2;
            let mut write_idx = code_idx_before_text + 1;
            self.code
                .insert(code_idx_before_text, Asm::BindVectorDescriptorSet(desc_set));
            while read_idx < self.code.len() {
                let (read_desc_set, read_start, read_end) =
                    self.code[read_idx].as_render_text().unwrap();
                let (write_desc_set, write_start, write_end) =
                    self.code[write_idx].as_render_text().unwrap();
                if read_desc_set != desc_set {
                    self.code
                        .insert(read_idx, Asm::BindVectorDescriptorSet(desc_set));
                    desc_set = read_desc_set;
                    read_idx += 1;
                    write_idx += 1;
                }

                if read_desc_set == write_desc_set && read_start == write_end {
                    self.code[write_idx] = Asm::RenderText(read_desc_set, write_start, read_end);
                } else {
                    write_idx += 1;
                }

                read_idx += 1;
            }

            // Trim off any excess rendering commands
            self.code.truncate(write_idx + 1);
        }

        self.vector_desc_sets += atlas.pages().len();

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            chars.cache.pending_write = Some(start..end);
            self.code.insert(0, Asm::WriteVectorVertices(idx));
        }

        // Handle copied ranges from earlier
        if !chars.cache.pending_copies.is_empty() {
            self.code.insert(0, Asm::CopyVectorVertices(idx));
        }

        // Transfer data from the previous GPU buffer to the new one, if we have a previous buffer
        if chars.cache.allocation.previous.is_some() {
            self.code.insert(0, Asm::TransferVectorData(idx));
        }

        // Add code to handle each of the copy-character-from-buffer-to-texture operations
        if atlas.has_pending_glyphs() {
            self.code.insert(0, Asm::CopyVectorGlyphs(idx));
        }

        end_idx
    }

    fn pipeline<T>(cmd: &Command<P, T>) -> Pipeline
    where
        T: AsRef<str>,
    {
        match cmd {
            Command::BitmapPosition(_) | Command::BitmapTransform(_) => Pipeline::Bitmap,
            _ => Pipeline::Vector,
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile`
    /// function.
    ///
    /// Must NOT be called before the previously drawn frame is completed.
    pub(super) fn reset(&mut self) {
        self.bitmap_fonts.clear();
        self.bitmap_desc_sets = 0;
        self.vector_desc_sets = 0;

        // TODO: Can these things be just two functions called two times each?

        // Advance the least-recently-used caching algorithm one step forward
        self.bitmap_chars
            .iter_mut()
            .for_each(|compilation| compilation.cache.reset());
        self.vector_chars
            .iter_mut()
            .for_each(|compilation| compilation.cache.reset());

        // Remove any fonts which are no longer in use
        self.bitmap_chars
            .retain(|compilation| !compilation.cache.items.is_empty());
        self.vector_chars
            .retain(|compilation| !compilation.cache.items.is_empty());
    }

    /// Sorts commands into a predictable and efficient order for drawing.
    fn sort_cmds<C, T>(cmds: &mut [C])
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
            // 2. Vector fonts (sorted by pointer)

            // TODO: Make this smaller with a well genericized function!
            match lhs {
                Command::BitmapPosition(lhs) => match rhs {
                    Command::BitmapPosition(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    Command::BitmapTransform(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    _ => Ordering::Less,
                },
                Command::BitmapTransform(lhs) => match rhs {
                    Command::BitmapPosition(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    Command::BitmapTransform(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    _ => Ordering::Less,
                },
                Command::VectorPosition(lhs) => match rhs {
                    Command::VectorPosition(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    Command::VectorTransform(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    _ => Ordering::Greater,
                },
                Command::VectorTransform(lhs) => match rhs {
                    Command::VectorPosition(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    Command::VectorTransform(rhs) => {
                        Shared::as_ptr(&lhs.font).cmp(&Shared::as_ptr(&rhs.font))
                    }
                    _ => Ordering::Greater,
                },
            }
        });
    }

    /// Sorts rendering code into optimal descriptor set/range ordering.
    fn sort_code(code: &mut [Asm]) {
        code.sort_unstable_by(|lhs, rhs| {
            let lhs = lhs.as_render_text().unwrap();
            let rhs = rhs.as_render_text().unwrap();

            match lhs.0.cmp(&rhs.0) {
                Ordering::Equal => lhs.1.cmp(&rhs.1),
                ne => ne,
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
            bitmap_chars: Default::default(),
            bitmap_desc_sets: 0,
            bitmap_fonts: Default::default(),
            code: Default::default(),
            vector_atlas: Default::default(),
            vector_chars: Default::default(),
            vector_desc_sets: 0,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Pipeline {
    Bitmap,
    Vector,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct VectorChar {
    char: char,
    size: u32,
    x: u32,
    y: u32,
}

impl VectorChar {
    /// Each character is rendered as a quad
    const STRIDE: u64 = 96;
}

impl Stride for VectorChar {
    fn stride(&self) -> u64 {
        Self::STRIDE
    }
}
