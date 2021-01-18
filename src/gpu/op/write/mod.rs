mod command;
mod compiler;
mod instruction;

pub use {command::Command as Write, compiler::Compiler};

use {
    self::instruction::Instruction,
    super::Op,
    crate::{
        color::TRANSPARENT_BLACK,
        gpu::{
            def::{
                push_const::WriteVertexPushConsts, ColorRenderPassMode, Graphics, GraphicsMode,
                RenderPassMode,
            },
            device,
            driver::{bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d},
            pool::{Lease, Pool},
            queue_mut, Texture2d,
        },
        math::{vec3, Area, CoordF, Mat4, RectF, Vec2},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    gfx_hal::{
        command::{
            CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, RenderAttachmentInfo,
            SubpassContents,
        },
        device::Device as _,
        format::Aspects,
        image::{
            Access, FramebufferAttachment, Layout, Offset, SubresourceLayers, Usage,
            ViewCapabilities,
        },
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        fmt::{Debug, Error, Formatter},
        iter::{empty, once},
        u8,
    },
};

#[cfg(feature = "blend-modes")]
use crate::gpu::{def::push_const::WriteFragmentPushConsts, BlendMode};

/// Describes the way `WriteOp` will write a given texture onto the destination texture.
#[derive(Clone, Copy, Hash, PartialEq)]
pub enum Mode {
    /// Blends source (a) with destination (b) using the given mode.
    #[cfg(feature = "blend-modes")]
    Blend((u8, BlendMode)),

    /// Writes source directly onto destination, with no blending.
    Texture,
}

/// Writes an iterator of source textures onto a destination texture, using optional modes.
///
/// `WriteOp` is intended to provide high speed image splatting for tile maps, bitmap drawing,
/// filtered image stretching, _etc..._ Although generic 3D transforms are offered, the main
/// use of this operation is 2D image composition.
///
/// _NOTE:_ When no image filtering or resizing is required the `CopyOp` may provide higher
/// performance. TODO: We can specialize for this so the API is the same, round the edge.
///
/// ## Examples
///
/// Writing a nine-sliced UI graphic:
///
/// ```
/// use screen_13::prelude_all::*;
///
/// ...
///
/// fn render_ui() {
///     // We've already sliced up a UI box image (🔪 top left, 🔪 top, 🔪 top right, ...)
///     let slices: [BitmapRef; 9] = ...
///     let render: &Render = ...
///
///     render.write().
///         .with_mode(WriteMode::Blend(0x7f, BlendMode::ColorDodge))
///         .with_preserve()
///         .record(&mut [
///             // top left
///             Write::tile_position(&slices[0], Area::new(0, 0, 32, 32), Coord::new(0, 0)),
///
///             // top
///             Write::tile_position(&slices[1], Area::new(32, 0, 384, 32), Coord::new(32, 0)),
///
///             // top right
///             Write::tile_position(&slices[2], Area::new(426, 0, 32, 32), Coord::new(426, 0)),
///
///             ...
///         ]);
/// }
/// ```
pub struct WriteOp<P>
where
    P: 'static + SharedPointerKind,
{
    back_buf: Lease<Shared<Texture2d, P>, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    compiler: Option<Lease<Compiler<P>, P>>,
    dst: Shared<Texture2d, P>,
    dst_preserve: bool,
    fence: Lease<Fence, P>,
    frame_buf: Option<Framebuffer2d>,
    graphics_texture: Option<Lease<Graphics, P>>,
    mode: Mode,

    #[cfg(feature = "debug-names")]
    name: String,

    pool: Option<Lease<Pool<P>, P>>,
}

impl<P> WriteOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        dst: &Shared<Texture2d, P>,
    ) -> Self {
        let mut cmd_pool = pool.cmd_pool();
        let dims = dst.dims();
        let fmt = dst.format();
        let fence = pool.fence(
            #[cfg(feature = "debug-names")]
            name,
        );

        Self {
            back_buf: pool.texture(
                #[cfg(feature = "debug-names")]
                &format!("{} backbuffer", name),
                dims,
                fmt,
                Layout::Undefined,
                Usage::COLOR_ATTACHMENT | Usage::INPUT_ATTACHMENT,
                1,
                1,
                1,
            ),
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            compiler: None,
            dst: Shared::clone(dst),
            dst_preserve: false,
            fence,
            frame_buf: None,
            graphics_texture: None,
            mode: Mode::Texture,
            #[cfg(feature = "debug-names")]
            name: name.to_owned(),
            pool: Some(pool),
        }
    }

    /// Sets the current write mode.
    #[must_use]
    pub fn with_mode(&mut self, mode: Mode) -> &mut Self {
        self.mode = mode;
        self
    }

    /// Preserves the contents of the destination texture. Without calling this function the
    /// existing contents of the destination texture will not be composited into the final result.
    #[must_use]
    pub fn with_preserve(&mut self) -> &mut Self {
        self.with_preserve_is(true)
    }

    /// Sets whether the destination texture will be composited into the final result or not.
    #[must_use]
    pub fn with_preserve_is(&mut self, val: bool) -> &mut Self {
        self.dst_preserve = val;
        self
    }

    /// Submits the given writes for hardware processing.
    pub fn record<W>(&mut self, writes: W)
    where
        W: IntoIterator<Item = Write<P>>,
    {
        unsafe {
            let mut pool = self.pool.as_mut().unwrap();
            let mut compiler = pool.write_compiler();
            {
                let mut instrs = compiler.compile(
                    #[cfg(feature = "debug-names")]
                    &self.name,
                    &mut pool,
                    writes,
                );

                let render_pass_mode = {
                    let fmt = self.dst.format();
                    let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
                        fmt,
                        preserve: self.dst_preserve,
                    });
                    let render_pass = pool.render_pass(render_pass_mode);
                    self.frame_buf.replace(Framebuffer2d::new(
                        #[cfg(feature = "debug-names")]
                        self.name.as_str(),
                        render_pass,
                        once(FramebufferAttachment {
                            format: fmt,
                            usage: Usage::COLOR_ATTACHMENT | Usage::INPUT_ATTACHMENT,
                            view_caps: ViewCapabilities::MUTABLE_FORMAT,
                        }),
                        self.dst.dims(),
                    ));
                    render_pass_mode
                };

                // Texture descriptors
                {
                    let descriptors = instrs.textures();
                    let desc_sets = descriptors.len();
                    if desc_sets > 0 {
                        const SUBPASS_IDX: u8 = 0;

                        let graphics_mode = match self.mode {
                            #[cfg(feature = "blend-modes")]
                            Mode::Blend((_, mode)) => GraphicsMode::Blend(mode),
                            Mode::Texture => GraphicsMode::Texture,
                        };
                        let mut graphics = pool.graphics_desc_sets(
                            #[cfg(feature = "debug-names")]
                            &self.name,
                            render_pass_mode,
                            SUBPASS_IDX,
                            graphics_mode,
                            desc_sets,
                        );
                        self.write_texture_descriptors(&mut graphics, descriptors);
                        self.graphics_texture = Some(graphics);
                    }
                }

                self.submit_begin();

                // Optional step: Fill dst into the backbuffer in order to preserve it in the output
                if self.dst_preserve {
                    self.submit_begin_preserve();
                }

                self.submit_begin_finish(render_pass_mode);

                while let Some(instr) = instrs.next() {
                    match instr {
                        Instruction::TextureDescriptors(desc_set) => {
                            self.submit_texture_descriptors(desc_set)
                        }
                        Instruction::TextureWrite(transform) => {
                            self.submit_texture_write(transform)
                        }
                    }
                }

                self.submit_finish();
            }

            self.compiler = Some(compiler);
        }
    }

    unsafe fn submit_begin(&mut self) {
        trace!("submit_begin");

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
    }

    unsafe fn submit_begin_preserve(&mut self) {
        trace!("submit_begin_preserve");

        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
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
                extent: self.dst.dims().as_extent_depth(1),
            }),
        );
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::ShaderReadOnlyOptimal,
            PipelineStage::FRAGMENT_SHADER,
            Access::SHADER_READ,
        );
    }

    unsafe fn submit_begin_finish(&mut self, render_pass_mode: RenderPassMode) {
        trace!("submit_begin_finish");

        let pool = self.pool.as_mut().unwrap();
        let render_pass = pool.render_pass(render_pass_mode);
        let graphics = self.graphics_texture.as_ref().unwrap();
        let rect = self.dst.dims().into();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            Access::COLOR_ATTACHMENT_WRITE,
        );
        // src.set_layout(
        //     &mut self.cmd_buf,
        //     Layout::ShaderReadOnlyOptimal,
        //     PipelineStage::FRAGMENT_SHADER,
        //     Access::SHADER_READ,
        // );
        self.cmd_buf.begin_render_pass(
            render_pass,
            self.frame_buf.as_ref().unwrap(),
            rect,
            once(RenderAttachmentInfo {
                image_view: self.back_buf.as_2d_color().as_ref(),
                clear_value: TRANSPARENT_BLACK.into(),
            }),
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.set_scissors(0, &[rect]);
        self.cmd_buf.set_viewports(0, &[viewport]);
        bind_graphics_descriptor_set(&mut self.cmd_buf, graphics.layout(), graphics.desc_set(0));
    }

    unsafe fn submit_texture_descriptors(&mut self, desc_set: usize) {
        trace!("submit_texture_descriptors");

        let graphics = self.graphics_texture.as_ref().unwrap();

        bind_graphics_descriptor_set(
            &mut self.cmd_buf,
            graphics.layout(),
            graphics.desc_set(desc_set),
        );
    }

    unsafe fn submit_texture_write(&mut self, transform: Mat4) {
        trace!("submit_texture_write");

        let graphics = self.graphics_texture.as_ref().unwrap();
        let offset = Vec2::zero();
        let scale = Vec2::one();

        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            WriteVertexPushConsts {
                offset,
                scale,
                transform,
            }
            .as_ref(),
        );

        #[cfg(feature = "blend-modes")]
        if let Mode::Blend((ab, _)) = self.mode {
            const RECIP: f32 = 1.0 / u8::MAX as f32;
            let ab = ab as f32 * RECIP;
            let ab_inv = 1.0 - ab;
            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::FRAGMENT,
                WriteVertexPushConsts::BYTE_LEN,
                WriteFragmentPushConsts { ab, ab_inv }.as_ref(),
            );
        }

        self.cmd_buf.draw(0..6, 0..1);
    }

    unsafe fn submit_finish(&mut self) {
        trace!("submit_finish");

        // End of the previous step...
        self.cmd_buf.end_render_pass();

        // Step 2: Copy the now-composited backbuffer to the `dst` texture
        self.back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
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
                extent: self.dst.dims().as_extent_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        queue_mut().submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&mut self.fence),
        );
    }

    unsafe fn write_texture_descriptors<'a, T>(&mut self, graphics: &mut Graphics, textures: T)
    where
        T: Iterator<Item = &'a Texture2d>,
    {
        trace!("write_texture_descriptors");

        #[cfg(feature = "blend-modes")]
        let dst_view = self.dst.as_2d_color();

        // Each source texture requres a unique descriptor set
        for (idx, texture) in textures.enumerate() {
            let (set, samplers) = graphics.desc_set_mut_with_samplers(idx);

            // A descriptor for this source texture
            device().write_descriptor_set(DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::CombinedImageSampler(
                    texture.as_2d_color().as_ref(),
                    Layout::ShaderReadOnlyOptimal,
                    samplers[0].as_ref(),
                )),
            });

            // Blend mode requires a descriptor for the destination texture
            #[cfg(feature = "blend-modes")]
            if let Mode::Blend(_) = self.mode {
                device().write_descriptor_set(DescriptorSetWrite {
                    set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        dst_view.as_ref(),
                        Layout::ShaderReadOnlyOptimal,
                        samplers[1].as_ref(),
                    )),
                });
            }
        }
    }
}

impl<P> Drop for WriteOp<P>
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

impl<P> Op<P> for WriteOp<P>
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool.take().unwrap()
    }

    unsafe fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
