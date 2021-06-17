mod bitmap_font;
mod command;
mod compiler;
mod instruction;
mod scalable_font;

pub use self::{
    bitmap_font::BitmapFont, command::Command, compiler::Compiler, scalable_font::ScalableFont,
};

use {
    self::instruction::{BitmapBindInstruction, Instruction, ScalableBindInstruction},
    super::{DataCopyInstruction, DataTransferInstruction, DataWriteInstruction, Op},
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            data::{CopyRange, CopyRangeDstRangeIter, CopyRangeSrcRangeIter},
            def::{
                push_const::{
                    BitmapFontVertexPushConsts, Mat4PushConst, Vec4PushConst, Vec4Vec4PushConst,
                },
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
            CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, RenderAttachmentInfo,
            SubpassContents,
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
        ops::{Deref, Range},
    },
};

pub const DEFAULT_SIZE: f32 = 32.0;
const FONT_VERTEX_SIZE: usize = 16;
const SUBPASS_IDX: u8 = 0;

/// Holds a reference to either a bitmapped or TrueType/Opentype font.
pub enum Font<'f, P>
where
    P: 'static + SharedPointerKind,
{
    /// A fixed-size bitmapped font as produced by programs compatible with the `.fnt` file format.
    ///
    /// **_NOTE:_** [BMFont](https://www.angelcode.com/products/bmfont/) is supported.
    Bitmap(&'f Shared<BitmapFont<P>, P>),

    /// A variable-size font.
    Scalable(&'f Shared<ScalableFont, P>),
}

impl<'f, P> Font<'f, P>
where
    P: SharedPointerKind,
{
    pub(super) fn as_bitmap(&'f self) -> Option<&'f Shared<BitmapFont<P>, P>> {
        match self {
            Self::Bitmap(font) => Some(font),
            _ => None,
        }
    }

    pub(super) fn as_scalable(&'f self) -> Option<&'f Shared<ScalableFont, P>> {
        match self {
            Self::Scalable(font) => Some(font),
            _ => None,
        }
    }

    pub(super) fn is_bitmap(&'f self) -> bool {
        self.as_bitmap().is_some()
    }

    pub(super) fn is_scalable(&'f self) -> bool {
        self.as_scalable().is_some()
    }
}

impl<'f, P> From<&'f Shared<BitmapFont<P>, P>> for Font<'f, P>
where
    P: SharedPointerKind,
{
    fn from(font: &'f Shared<BitmapFont<P>, P>) -> Self {
        Self::Bitmap(font)
    }
}

impl<'f, P> From<&'f Shared<ScalableFont, P>> for Font<'f, P>
where
    P: SharedPointerKind,
{
    fn from(font: &'f Shared<ScalableFont, P>) -> Self {
        Self::Scalable(font)
    }
}

/// A container of graphics types and the functions which enable the recording and submission of
/// font operations.
///
/// Supports bitmapped fonts and scalable fonts. Bitmapped fonts may either be the one-color (glyph)
/// type or the two-color (outline) type.
pub struct TextOp<P>
where
    P: 'static + SharedPointerKind,
{
    back_buf: Lease<Shared<Texture2d, P>, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    compiler: Option<Lease<Compiler<P>, P>>,
    dst: Shared<Texture2d, P>,
    fence: Lease<Fence, P>,
    frame_buf: Framebuffer2d,
    graphics_bitmap_glyph: Option<Lease<Graphics, P>>,
    graphics_bitmap_outline: Option<Lease<Graphics, P>>,
    graphics_scalable: Option<Lease<Graphics, P>>,

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
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
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
                    | ImageUsage::INPUT_ATTACHMENT
                    | ImageUsage::TRANSFER_DST
                    | ImageUsage::TRANSFER_SRC,
                view_caps: ViewCapabilities::MUTABLE_FORMAT,
            }),
            dims,
        );

        Self {
            back_buf,
            cmd_buf,
            cmd_pool,
            compiler: None,
            dst: Shared::clone(dst),
            fence,
            frame_buf,
            graphics_bitmap_glyph: None,
            graphics_bitmap_outline: None,
            graphics_scalable: None,

            #[cfg(feature = "debug-names")]
            name: name.to_owned(),

            pool: Some(pool),
        }
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

                // Texture descriptors for one-color glyph bitmap fonts
                {
                    let descriptors = instrs.bitmap_glyph_descriptors();
                    let desc_sets = descriptors.len();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::BitmapGlyph),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, descriptors);
                        self.graphics_bitmap_glyph = Some(graphics);
                    }
                }

                // Texture descriptors for two-color outline bitmap fonts
                {
                    let descriptors = instrs.bitmap_outline_descriptors();
                    let desc_sets = descriptors.len();
                    if desc_sets > 0 {
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            GraphicsMode::Font(FontMode::BitmapOutline),
                            desc_sets,
                        );
                        Self::write_texture_descriptors(&mut graphics, descriptors);
                        self.graphics_bitmap_outline = Some(graphics);
                    }
                }

                self.submit_begin(dims);

                while let Some(instr) = instrs.next() {
                    match instr {
                        Instruction::BitmapGlyphBegin => self.submit_bitmap_glyph_begin(),
                        Instruction::BitmapGlyphBind(instr) => self.submit_bitmap_glyph_bind(instr),
                        Instruction::BitmapGlyphColor(glyph_color) => {
                            self.submit_bitmap_glyph_color(glyph_color)
                        }
                        Instruction::BitmapGlyphTransform(view_proj) => {
                            self.submit_bitmap_glyph_transform(view_proj)
                        }
                        Instruction::BitmapOutlineBegin => self.submit_bitmap_outline_begin(),
                        Instruction::BitmapOutlineBind(instr) => {
                            self.submit_bitmap_outline_bind(instr)
                        }
                        Instruction::BitmapOutlineColors(glyph_color, outline_color) => {
                            self.submit_bitmap_outline_colors(glyph_color, outline_color)
                        }
                        Instruction::BitmapOutlineTransform(view_proj) => {
                            self.submit_bitmap_outline_transform(view_proj)
                        }
                        Instruction::DataTransfer(instr) => self.submit_data_transfer(instr),
                        Instruction::ScalableBegin => self.submit_scalable_begin(),
                        Instruction::ScalableBind(instr) => self.submit_scalable_bind(instr),
                        Instruction::ScalableColor(glyph_color) => {
                            self.submit_scalable_color(glyph_color)
                        }
                        Instruction::ScalableTransform(view_proj) => {
                            self.submit_scalable_transform(view_proj)
                        }
                        Instruction::RenderBegin => {
                            self.submit_render_begin(dims, render_pass_mode)
                        }
                        Instruction::RenderText(range) => self.submit_render_text(range),
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

    unsafe fn submit_bitmap_glyph_begin(&mut self) {
        trace!("submit_bitmap_glyph_begin");

        let graphics = self.graphics_bitmap_glyph.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_bitmap_glyph_bind(&mut self, instr: BitmapBindInstruction<'_, P>) {
        trace!("submit_bitmap_glyph_bind");

        let graphics = self.graphics_bitmap_glyph.as_ref().unwrap();
        let desc_set = graphics.desc_set(instr.desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
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
        instr.buf.barrier_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
            BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
            0..instr.buf_len,
        );
    }

    unsafe fn submit_bitmap_glyph_color(&mut self, glyph_color: AlphaColor) {
        trace!("submit_bitmap_glyph_color");

        let graphics = self.graphics_bitmap_glyph.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            BitmapFontVertexPushConsts::BYTE_LEN,
            Vec4PushConst {
                val: glyph_color.to_rgba(),
            }
            .as_ref(),
        );
    }

    unsafe fn submit_bitmap_glyph_transform(&mut self, view_proj: Mat4) {
        trace!("submit_bitmap_glyph_transform");

        let dims: CoordF = self.dst.dims().into();
        let dims_inv = 1.0 / dims;
        let graphics = self.graphics_bitmap_glyph.as_ref().unwrap();
        let layout = graphics.layout();
        let mut push_consts = BitmapFontVertexPushConsts::default();
        push_consts.dims_inv = dims_inv.into();
        push_consts.view_proj = view_proj;

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            push_consts.as_ref(),
        );
    }

    unsafe fn submit_bitmap_outline_begin(&mut self) {
        trace!("submit_bitmap_outline_begin");

        let graphics = self.graphics_bitmap_outline.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_bitmap_outline_bind(&mut self, instr: BitmapBindInstruction<'_, P>) {
        trace!("submit_bitmap_outline_bind");

        let graphics = self.graphics_bitmap_outline.as_ref().unwrap();
        let desc_set = graphics.desc_set(instr.desc_set);
        let layout = graphics.layout();

        bind_graphics_descriptor_set(&mut self.cmd_buf, layout, desc_set);
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
        instr.buf.barrier_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
            BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
            0..instr.buf_len,
        );
    }

    unsafe fn submit_bitmap_outline_colors(
        &mut self,
        glyph_color: AlphaColor,
        outline_color: AlphaColor,
    ) {
        trace!("submit_bitmap_outline_colors");

        let graphics = self.graphics_bitmap_outline.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            BitmapFontVertexPushConsts::BYTE_LEN,
            Vec4Vec4PushConst {
                val: [glyph_color.to_rgba(), outline_color.to_rgba()],
            }
            .as_ref(),
        );
    }

    unsafe fn submit_bitmap_outline_transform(&mut self, view_proj: Mat4) {
        trace!("submit_bitmap_outline_transform");

        let dims: CoordF = self.dst.dims().into();
        let dims_inv = 1.0 / dims;
        let graphics = self.graphics_bitmap_outline.as_ref().unwrap();
        let layout = graphics.layout();
        let mut push_consts = BitmapFontVertexPushConsts::default();
        push_consts.dims_inv = dims_inv.into();
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

    unsafe fn submit_render_begin(&mut self, dims: Extent, render_pass_mode: RenderPassMode) {
        trace!("submit_render_begin");

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

    unsafe fn submit_render_text(&mut self, vertices: Range<VertexCount>) {
        trace!("submit_render_text");

        self.cmd_buf.draw(vertices, 0..1);
    }

    unsafe fn submit_scalable_begin(&mut self) {
        trace!("submit_scalable_begin");

        let graphics = self.graphics_scalable.as_ref().unwrap();

        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
    }

    unsafe fn submit_scalable_bind(&mut self, instr: ScalableBindInstruction<'_, P>) {
        trace!("submit_scalable_bind");

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

    unsafe fn submit_scalable_color(&mut self, glyph_color: AlphaColor) {
        trace!("submit_scalable_color");

        let graphics = self.graphics_scalable.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::FRAGMENT,
            Mat4PushConst::BYTE_LEN,
            Vec4PushConst {
                val: glyph_color.to_rgba(),
            }
            .as_ref(),
        );
    }

    unsafe fn submit_scalable_transform(&mut self, val: Mat4) {
        trace!("submit_scalable_transform");

        let graphics = self.graphics_scalable.as_ref().unwrap();
        let layout = graphics.layout();

        self.cmd_buf.push_graphics_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            Mat4PushConst { val }.as_ref(),
        );
    }

    unsafe fn submit_vertex_copies(&mut self, instr: DataCopyInstruction) {
        trace!("submit_vertex_copies");

        instr.buf.barrier_ranges(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_WRITE..BufferAccess::TRANSFER_READ,
            CopyRangeSrcRangeIter(instr.ranges.iter()),
        );
        instr.buf.barrier_ranges(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_WRITE..BufferAccess::TRANSFER_WRITE,
            CopyRangeDstRangeIter(instr.ranges.iter()),
        );
        instr.buf.copy_ranges(&mut self.cmd_buf, instr.ranges);
        instr.buf.barrier_ranges(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER..PipelineStage::VERTEX_INPUT,
            BufferAccess::TRANSFER_WRITE..BufferAccess::VERTEX_BUFFER_READ,
            CopyRangeDstRangeIter(instr.ranges.iter()),
        );
    }

    unsafe fn submit_vertex_write(&mut self, instr: DataWriteInstruction) {
        trace!("submit_vertex_write");

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

    unsafe fn write_texture_descriptors<I, T>(graphics: &mut Graphics, textures: I)
    where
        I: Iterator<Item = T>,
        T: Deref<Target = Texture2d>,
    {
        for (idx, tex) in textures.enumerate() {
            let (set, samplers) = graphics.desc_set_mut_with_samplers(idx);

            device().write_descriptor_set(DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::CombinedImageSampler(
                    tex.as_2d_color().as_ref(),
                    Layout::General,
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
