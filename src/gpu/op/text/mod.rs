mod bitmap_font;
mod command;
mod compiler;
mod dyn_atlas;
mod glyph;
mod instruction;
mod vector_font;

pub use self::{
    bitmap_font::BitmapFont,
    command::{BitmapCommand, Command, VectorCommand},
    compiler::Compiler,
    vector_font::{VectorFont, VectorFontSettings},
};

use {
    self::{
        dyn_atlas::DynamicAtlas,
        instruction::{Instruction, VertexBindInstruction},
    },
    super::{DataCopyInstruction, DataTransferInstruction, DataWriteInstruction, Op},
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            data::{CopyRange, CopyRangeDstRangeIter, CopyRangeSrcRangeIter},
            def::{
                push_const::{FontVertexPushConsts, Vec4PushConst, Vec4Vec4PushConst},
                ColorRenderPassMode, FontMode, Graphics, GraphicsMode, RenderPassMode,
            },
            device,
            driver::{bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d},
            pool::{Lease, Pool},
            queue_mut, Texture2d,
        },
        math::{CoordF, Extent, Mat4},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange},
        command::{
            BufferImageCopy, CommandBuffer as _, CommandBufferFlags, ImageCopy, Level,
            RenderAttachmentInfo, SubpassContents,
        },
        device::Device as _,
        format::Aspects,
        image::{
            Access as ImageAccess, FramebufferAttachment, Layout, Offset, SubresourceLayers,
            Usage as ImageUsage, ViewCapabilities,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::Queue as _,
        Backend, VertexCount,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        borrow::Borrow,
        iter::{empty, once},
        ops::Range,
    },
};

const SUBPASS_IDX: u8 = 0;

/// A container of graphics types and the functions which enable the recording and submission of
/// font operations.
///
/// Supports bitmapped fonts and scalable fonts. Bitmapped fonts may either be the one-color (glyph)
/// type or the two-color (outline) type.
pub struct TextOp<P>
where
    P: 'static + SharedPointerKind,
{
    atlas_buf_len: u64,
    atlas_dims: u32,
    back_buf: Lease<Shared<Texture2d, P>, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    compiler: Option<Lease<Compiler<P>, P>>,
    dst: Shared<Texture2d, P>,
    fence: Lease<Fence, P>,
    frame_buf: Framebuffer2d,
    graphics_bitmap: Option<Lease<Graphics, P>>,
    graphics_vector: Option<Lease<Graphics, P>>,

    #[cfg(feature = "debug-names")]
    name: String,

    pool: Option<Lease<Pool<P>, P>>,
}

impl<P> TextOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        dst: &Shared<Texture2d, P>,
    ) -> Self {
        let dims = dst.dims();
        let fmt = dst.format();

        let back_buf = pool.texture(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let mut cmd_pool = pool.cmd_pool();
        let cmd_buf = cmd_pool.allocate_one(Level::Primary);
        let fence = pool.fence(
            #[cfg(feature = "debug-names")]
            name,
        );

        // Setup the framebuffer
        let frame_buf = Framebuffer2d::new(
            #[cfg(feature = "debug-names")]
            self.name.as_str(),
            pool.render_pass(RenderPassMode::Color(ColorRenderPassMode {
                fmt,
                preserve: true,
            })),
            once(FramebufferAttachment {
                format: fmt,
                usage: ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::TRANSFER_DST
                    | ImageUsage::TRANSFER_SRC,
                view_caps: ViewCapabilities::MUTABLE_FORMAT,
            }),
            dims,
        );

        Self {
            atlas_buf_len: 16384,
            atlas_dims: 4096,
            back_buf,
            cmd_buf,
            cmd_pool,
            compiler: None,
            dst: Shared::clone(dst),
            fence,
            frame_buf,
            graphics_bitmap: None,
            graphics_vector: None,

            #[cfg(feature = "debug-names")]
            name: name.to_owned(),

            pool: Some(pool),
        }
    }

    /// Specifies a specific vector font dynamic atlas buffer size to use for this operation.
    ///
    /// **_NOTE:_** This is an advanced option and almost never needs to be used. The default value
    /// will suffice for nearly all use cases. The buffer in question is used to copy rasterized
    /// characters into a GPU atlas texture and will be larger than the value specified here if
    /// needed. Additionally, if the length is not suffucient then additional buffers will be used.
    ///
    /// **_NOTE:_** One method of finding out if you need to set this value is to turn on the
    /// `debug-names` feature and profile your program. If you see multiple `Vector font buffer`
    /// objects used during a a single text operation you _may_ benefit from a larger value.
    ///
    /// The default value is 16384.
    pub fn with_atlas_buf_len(&mut self, len: u64) -> &mut Self {
        self.atlas_buf_len = len;
        self
    }

    /// Specifies a specific vector font atlas texture size to use for this operation.
    ///
    /// **_NOTE:_** This is an advanced option and almost never needs to be used. Atlas dimensions
    /// will be determined automatically based on device limits and may override any value set here.
    /// If an atlas cannot fit all required characters then additional atlases will be used.
    /// Additionally, if a particular character is larger than this size a larger atlas will be
    /// used.
    ///
    /// The default value is 4096.
    pub fn with_atlas_dims(&mut self, dim: u32) -> &mut Self {
        // TODO: Use actual maxImageDimension2D from device limits!
        self.atlas_dims = dim.min(32768);
        self
    }

    /// Submits the given commands for hardware processing.
    ///
    /// **_NOTE:_** Individual commands within this batch will have an unstable draw order. For a
    /// stable draw order submit additional batches.
    pub fn record<C, T>(&mut self, cmds: &mut [C])
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        unsafe {
            let pool = self.pool.as_mut().unwrap();
            let mut compiler = pool.text_compiler();
            {
                let dims = self.dst.dims();
                let mut instrs = compiler.compile(
                    #[cfg(feature = "debug-names")]
                    &self.name,
                    pool,
                    cmds,
                    dims.into(),
                    self.atlas_buf_len,
                    self.atlas_dims,
                );

                // Early-out if no text will be rendered (empty cmds or strings or auto-culled)
                if instrs.is_empty() {
                    return;
                }

                let fmt = self.dst.format();
                let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
                    fmt,
                    preserve: true,
                });

                // Texture descriptors for bitmap fonts
                {
                    let desc_sets = instrs.bitmap_desc_sets();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::Bitmap),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, instrs.bitmap_textures());
                        self.graphics_bitmap = Some(graphics);
                    }
                }

                // Texture descriptors for vector fonts
                {
                    let desc_sets = instrs.vector_desc_sets();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::Vector),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, instrs.vector_textures());
                        self.graphics_vector = Some(graphics);
                    }
                }

                self.submit_begin(dims);

                while let Some(instr) = instrs.next() {
                    match instr {
                        Instruction::BitmapBegin => self.submit_bitmap_begin(),
                        Instruction::BitmapBindDescriptorSet(desc_set) => {
                            self.submit_bitmap_bind_desc_set(desc_set)
                        }
                        Instruction::BitmapColors(glyph, outline) => {
                            self.submit_bitmap_colors(glyph, outline)
                        }
                        Instruction::BitmapTransform(view_proj) => {
                            self.submit_bitmap_transform(view_proj)
                        }
                        Instruction::DataTransfer(instr) => self.submit_data_transfer(instr),
                        Instruction::VectorBegin => self.submit_vector_begin(),
                        Instruction::VectorBindDescriptorSet(desc_set) => {
                            self.submit_vector_bind_desc_set(desc_set)
                        }
                        Instruction::VectorColor(glyph_color) => {
                            self.submit_vector_color(glyph_color)
                        }
                        Instruction::VectorGlyphCopy(atlas) => {
                            self.submit_vector_glyph_copies(atlas)
                        }
                        Instruction::VectorTransform(view_proj) => {
                            self.submit_vector_transform(view_proj)
                        }
                        Instruction::TextBegin => self.submit_text_begin(dims, render_pass_mode),
                        Instruction::TextRender(range) => self.submit_text_render(range),
                        Instruction::VertexBind(instr) => self.submit_vertex_bind(instr),
                        Instruction::VertexCopy(instr) => self.submit_vertex_copies(instr),
                        Instruction::VertexWrite(instr) => self.submit_vertex_write(instr),
                    }
                }

                self.submit_finish(dims);
            }

            self.compiler = Some(compiler);
        }
    }

    unsafe fn submit_begin(&mut self, dims: Extent) {
        trace!("submit_begin");

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy dst into the backbuffer
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            self.dst.as_ref(),
            Layout::TransferSrcOptimal,
            self.back_buf.as_ref(),
            Layout::TransferDstOptimal,
            once(ImageCopy {
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset: Offset::ZERO,
                dst_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                dst_offset: Offset::ZERO,
                extent: dims.as_extent_depth(1),
            }),
        );

        // Step 2: Setup for drawing the fonts into the backbuffer
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_READ | ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
    }

    unsafe fn submit_bitmap_begin(&mut self) {
        trace!("submit_bitmap_begin");

        let graphics = self.graphics_bitmap.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_bitmap_bind_desc_set(&mut self, desc_set: usize) {
        trace!("submit_bitmap_bind_desc_set");

        let graphics = self.graphics_bitmap.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_bitmap_colors(&mut self, glyph: AlphaColor, outline: AlphaColor) {
        trace!("submit_bitmap_colors");

        let graphics = self.graphics_bitmap.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            FontVertexPushConsts::BYTE_LEN,
            Vec4Vec4PushConst {
                val: [glyph.to_rgba(), outline.to_rgba()],
            }
            .as_ref(),
        );
    }

    unsafe fn submit_bitmap_transform(&mut self, view_proj: Mat4) {
        trace!("submit_bitmap_transform");

        let dims: CoordF = self.dst.dims().into();
        let graphics = self.graphics_bitmap.as_ref().unwrap();
        let layout = graphics.layout();
        let mut push_consts = FontVertexPushConsts::default();
        push_consts.dims = dims.into();
        push_consts.view_proj = view_proj;

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            push_consts.as_ref(),
        );
    }

    unsafe fn submit_data_transfer(&mut self, instr: DataTransferInstruction) {
        trace!(
            "submit_data_transfer {}..{}",
            instr.src_range.start,
            instr.src_range.end
        );

        instr.src.transfer_range(
            &mut self.cmd_buf,
            instr.dst,
            CopyRange {
                src: instr.src_range.clone(),
                dst: 0,
            },
        );
        instr.dst.barrier_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
            BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
            0..instr.src_range.end - instr.src_range.start,
        );
    }

    unsafe fn submit_text_begin(&mut self, dims: Extent, render_pass_mode: RenderPassMode) {
        trace!("submit_text_begin");

        let pool = self.pool.as_mut().unwrap();
        let render_pass = pool.render_pass(render_pass_mode);
        let rect = dims.as_rect();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        self.cmd_buf.begin_render_pass(
            render_pass,
            &self.frame_buf,
            rect,
            once(RenderAttachmentInfo {
                image_view: self.back_buf.as_2d_color().as_ref(),
                clear_value: TRANSPARENT_BLACK.into(),
            }),
            SubpassContents::Inline,
        );
        self.cmd_buf.set_scissors(0, once(rect));
        self.cmd_buf.set_viewports(0, once(viewport));
    }

    unsafe fn submit_text_render(&mut self, vertices: Range<VertexCount>) {
        trace!("submit_text_render {}..{}", vertices.start, vertices.end);

        self.cmd_buf.draw(vertices, 0..1);
    }

    unsafe fn submit_vector_begin(&mut self) {
        trace!("submit_vector_begin");

        let graphics = self.graphics_vector.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_vector_bind_desc_set(&mut self, desc_set: usize) {
        trace!("submit_vector_bind_desc_set");

        let graphics = self.graphics_vector.as_ref().unwrap();
        let desc_set = graphics.desc_set(desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
    }

    unsafe fn submit_vector_color(&mut self, glyph_color: AlphaColor) {
        trace!("submit_vector_color");

        let graphics = self.graphics_vector.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            FontVertexPushConsts::BYTE_LEN,
            Vec4PushConst {
                val: glyph_color.to_rgba(),
            }
            .as_ref(),
        );
    }

    unsafe fn submit_vector_glyph_copies(&mut self, atlas: &mut DynamicAtlas<P>) {
        trace!("submit_vector_glyph_copies");

        while let Some(glyph) = atlas.pop_pending_glyph() {
            glyph
                .buf
                .write_range(&mut self.cmd_buf, glyph.buf_range.clone());
            glyph.buf.barrier_range(
                &mut self.cmd_buf,
                PipelineStage::TRANSFER..PipelineStage::TRANSFER,
                BufferAccess::TRANSFER_WRITE..BufferAccess::TRANSFER_READ,
                glyph.buf_range.clone(),
            );
            glyph.page.set_layout(
                &mut self.cmd_buf,
                Layout::TransferDstOptimal,
                PipelineStage::TRANSFER,
                ImageAccess::TRANSFER_WRITE,
            );
            self.cmd_buf.copy_buffer_to_image(
                glyph.buf.as_ref(),
                glyph.page.as_ref(),
                Layout::TransferDstOptimal,
                once(BufferImageCopy {
                    buffer_offset: glyph.buf_range.start,
                    buffer_width: glyph.page_rect.dims.x,
                    buffer_height: glyph.page_rect.dims.y,
                    image_layers: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: glyph.page_rect.pos.into(),
                    image_extent: glyph.page_rect.dims.as_extent_depth(1),
                }),
            );
            glyph.page.set_layout(
                &mut self.cmd_buf,
                Layout::ShaderReadOnlyOptimal,
                PipelineStage::VERTEX_SHADER,
                ImageAccess::SHADER_READ,
            );
        }
    }

    unsafe fn submit_vector_transform(&mut self, view_proj: Mat4) {
        trace!("submit_vector_transform");

        let dims: CoordF = self.dst.dims().into();
        let graphics = self.graphics_vector.as_ref().unwrap();
        let layout = graphics.layout();
        let mut push_consts = FontVertexPushConsts::default();
        push_consts.dims = dims.into();
        push_consts.view_proj = view_proj;

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            push_consts.as_ref(),
        );
    }

    unsafe fn submit_vertex_bind(&mut self, instr: VertexBindInstruction<'_, P>) {
        trace!("submit_vertex_bind");

        self.cmd_buf.bind_vertex_buffers(
            0,
            once((
                instr.buf.as_ref(),
                SubRange {
                    offset: 0,
                    size: Some(instr.buf_len),
                },
            )),
        );
    }

    unsafe fn submit_vertex_copies(&mut self, instr: DataCopyInstruction) {
        trace!("submit_vertex_copies");

        // Vertex ranges overlap and so must be submitted individually
        for range in instr.ranges {
            trace!(
                "submit_vertex_copies {}..{} -> {}..{} ({} bytes)",
                range.src.start,
                range.src.end,
                range.dst,
                range.dst + range.src.end - range.src.start,
                range.src.end - range.src.start
            );

            instr.buf.barrier_ranges(
                &mut self.cmd_buf,
                PipelineStage::TRANSFER..PipelineStage::TRANSFER,
                BufferAccess::TRANSFER_WRITE..BufferAccess::TRANSFER_READ,
                CopyRangeSrcRangeIter(once(range)),
            );
            instr.buf.barrier_ranges(
                &mut self.cmd_buf,
                PipelineStage::TRANSFER..PipelineStage::TRANSFER,
                BufferAccess::TRANSFER_WRITE..BufferAccess::TRANSFER_WRITE,
                CopyRangeDstRangeIter(once(range)),
            );
            instr.buf.copy_range(&mut self.cmd_buf, range);
            instr.buf.barrier_ranges(
                &mut self.cmd_buf,
                PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
                BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
                CopyRangeDstRangeIter(once(range)),
            );
        }
    }

    unsafe fn submit_vertex_write(&mut self, instr: DataWriteInstruction) {
        trace!(
            "submit_vertex_write {}..{} ({} bytes)",
            instr.range.start,
            instr.range.end,
            instr.range.end - instr.range.start
        );

        instr.buf.barrier_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_READ..BufferAccess::TRANSFER_WRITE,
            instr.range.start..instr.range.end,
        );
        instr
            .buf
            .write_range(&mut self.cmd_buf, instr.range.start..instr.range.end);
        instr.buf.barrier_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
            BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
            instr.range.start..instr.range.end,
        );
    }

    unsafe fn submit_finish(&mut self, dims: Extent) {
        trace!("submit_finish");

        // Copy the backbuffer into dst
        self.cmd_buf.end_render_pass();
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            self.back_buf.as_ref(),
            Layout::TransferSrcOptimal,
            self.dst.as_ref(),
            Layout::TransferDstOptimal,
            once(ImageCopy {
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset: Offset::ZERO,
                dst_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                dst_offset: Offset::ZERO,
                extent: dims.as_extent_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        queue_mut().submit(once(&self.cmd_buf), empty(), empty(), Some(&mut self.fence));
    }

    unsafe fn write_texture_descriptors<'i, I>(graphics: &mut Graphics, textures: I)
    where
        I: Iterator<Item = &'i Texture2d>,
    {
        for (idx, tex) in textures.enumerate() {
            let (set, samplers) = graphics.desc_set_mut_with_samplers(idx);

            device().write_descriptor_set(DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::CombinedImageSampler(
                    tex.as_2d_color().as_ref(),
                    Layout::ShaderReadOnlyOptimal,
                    samplers[0].as_ref(),
                )),
            });
        }
    }
}

impl<P> Drop for TextOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait();
        }

        // Causes the compiler to drop internal caches which store texture refs; they were being
        // held alive there so that they could not be dropped until we finished GPU execution
        if let Some(compiler) = self.compiler.as_mut() {
            compiler.reset();
        }
    }
}

impl<P> Op<P> for TextOp<P>
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn is_complete(&self) -> bool {
        Fence::status(&self.fence)
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool.take().unwrap()
    }

    unsafe fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
