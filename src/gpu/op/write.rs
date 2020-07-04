use {
    super::{mat4_to_u32_array, wait_for_fence, Op},
    crate::{
        color::TRANSPARENT_BLACK,
        gpu::{
            driver::{
                bind_graphics_descriptor_set, CommandPool, Driver, Fence, Framebuffer2d, Image2d,
                PhysicalDevice,
            },
            pool::{Graphics, GraphicsMode, Lease, RenderPassMode},
            PoolRef, TextureRef,
        },
        math::Mat4,
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        device::Device,
        format::Aspects,
        image::{Access, Layout, Offset, SubresourceLayers, Tiling, Usage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::{empty, once},
        u8,
    },
};

const QUEUE_TYPE: QueueType = QueueType::Graphics;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlendMode {
    Add,
    Alpha,
    ColorBurn,
    ColorDodge,
    Color,
    Darken,
    DarkenColor,
    Difference,
    Divide,
    Exclusion,
    HardLight,
    HardMix,
    LinearBurn,
    Multiply,
    Normal,
    Overlay,
    Screen,
    Subtract,
    VividLight,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum Mode {
    Blend((u8, BlendMode)),
    Texture,
}

pub struct WriteOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    back_buf: Lease<TextureRef<Image2d>>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: TextureRef<D>,
    dst_layout: Option<(Layout, PipelineStage, Access)>,
    fence: Lease<Fence>,
    frame_buf: Option<Framebuffer2d>,
    graphics: Option<(Mode, Lease<Graphics>)>,
    #[cfg(debug_assertions)]
    name: String,
    pool: PoolRef,
    preserve_dst: bool,
    src: TextureRef<S>,
    transform: Mat4,
}

// TODO: Make this take a list of arguments, not just one texture/mode/transform
impl<S, D> WriteOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        src: &TextureRef<S>,
        dst: &TextureRef<D>,
    ) -> Self {
        let mut pool_ref = pool.borrow_mut();
        let family = pool_ref.driver().borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);

        Self {
            back_buf: pool_ref.texture(
                #[cfg(debug_assertions)]
                &format!("{} backbuffer", name),
                dst.borrow().dims(),
                Tiling::Optimal,
                dst.borrow().format(),
                Layout::Undefined,
                Usage::COLOR_ATTACHMENT
                    | Usage::INPUT_ATTACHMENT
                    | Usage::TRANSFER_DST
                    | Usage::TRANSFER_SRC,
                1,
                1,
                1,
            ),
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            dst: TextureRef::clone(dst),
            dst_layout: None,
            fence: pool_ref.fence(),
            frame_buf: None,
            graphics: None,
            #[cfg(debug_assertions)]
            name: name.to_owned(),
            pool: PoolRef::clone(pool),
            preserve_dst: false,
            src: TextureRef::clone(src),
            transform: Mat4::identity(),
        }
    }

    pub fn with_dst_layout(mut self, layout: Layout, stage: PipelineStage, access: Access) -> Self {
        self.dst_layout = Some((layout, stage, access));
        self
    }

    pub fn with_mode(mut self, mode: Mode) -> Self {
        unsafe {
            self.set_mode(mode);
        }
        self
    }

    pub fn with_preserve_dst(mut self) -> Self {
        self.preserve_dst = true;

        // Graphics mode relies on preserve_dst to determine render pass, so we must update it
        let mode = if let Some((mode, _)) = self.graphics.as_ref() {
            Some(*mode)
        } else {
            None
        };
        if let Some(mode) = mode {
            unsafe {
                self.set_mode(mode);
            }
        }

        self
    }

    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub fn record(mut self) -> impl Op {
        // If no write mode was selected we use `Copy`
        if self.graphics.is_none() {
            //self.set_mode(Mode::Fill);
            todo!();
        }

        // Setup the framebuffer
        {
            let mut pool = self.pool.borrow_mut();
            let driver = Driver::clone(pool.driver());
            self.frame_buf.replace(Framebuffer2d::new(
                Driver::clone(&driver),
                pool.render_pass(self.render_pass_mode()),
                once(self.back_buf.borrow().as_default_2d_view().as_ref()),
                self.dst.borrow().dims(),
            ));
        }

        unsafe {
            self.write_descriptor_sets();
            self.submit();
        };

        WriteResult { op: self }
    }

    fn render_pass_mode(&self) -> RenderPassMode {
        if self.preserve_dst {
            RenderPassMode::ReadWrite
        } else {
            RenderPassMode::Write
        }
    }

    unsafe fn set_mode(&mut self, mode: Mode) {
        let graphics_mode = match mode {
            Mode::Blend((_, mode)) => GraphicsMode::Blend(mode),
            Mode::Texture => GraphicsMode::Texture,
        };
        self.graphics = Some((
            mode,
            self.pool.borrow_mut().graphics(
                #[cfg(debug_assertions)]
                &self.name,
                graphics_mode,
                self.render_pass_mode(),
                0,
            ),
        ));
    }

    unsafe fn submit(&mut self) {
        let mut pool = self.pool.borrow_mut();
        let (mode, graphics) = self.graphics.as_ref().unwrap();
        let mut back_buf = self.back_buf.borrow_mut();
        let mut src = self.src.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let dims = dst.dims();
        let rect = dims.as_rect();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Optional step: Fill dst into the backbuffer in order to preserve it in the output
        if self.preserve_dst {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::TransferSrcOptimal,
                PipelineStage::TRANSFER,
                Access::TRANSFER_READ,
            );
            back_buf.set_layout(
                &mut self.cmd_buf,
                Layout::TransferDstOptimal,
                PipelineStage::TRANSFER,
                Access::TRANSFER_WRITE,
            );
            self.cmd_buf.copy_image(
                dst.as_ref(),
                Layout::TransferSrcOptimal,
                back_buf.as_ref(),
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
                    extent: dims.as_extent(1),
                }),
            );
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::ShaderReadOnlyOptimal,
                PipelineStage::FRAGMENT_SHADER,
                Access::SHADER_READ,
            );
        }

        // Step 1: Write src into the backbuffer, but blending using our shader `mode`
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            Access::COLOR_ATTACHMENT_WRITE,
        );
        src.set_layout(
            &mut self.cmd_buf,
            Layout::ShaderReadOnlyOptimal,
            PipelineStage::FRAGMENT_SHADER,
            Access::SHADER_READ,
        );
        self.cmd_buf.begin_render_pass(
            pool.render_pass(self.render_pass_mode()),
            self.frame_buf.as_ref().unwrap(),
            rect,
            &[TRANSPARENT_BLACK.into()],
            SubpassContents::Inline,
        );
        self.cmd_buf.bind_graphics_pipeline(graphics.pipeline());
        self.cmd_buf.push_graphics_constants(
            graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            &mat4_to_u32_array(self.transform),
        );
        if let Mode::Blend((ab, _)) = mode {
            let ab = *ab as f32 / u8::MAX as f32;
            self.cmd_buf.push_graphics_constants(
                graphics.layout(),
                ShaderStageFlags::FRAGMENT,
                64,
                &[(ab).to_bits(), (1.0 - ab).to_bits()],
            );
        }
        bind_graphics_descriptor_set(&mut self.cmd_buf, graphics.layout(), graphics.desc_set(0));
        self.cmd_buf.set_scissors(0, &[rect]);
        self.cmd_buf.set_viewports(0, &[viewport]);
        self.cmd_buf.draw(0..6, 0..1);
        self.cmd_buf.end_render_pass();

        // Step 2: Copy the now-composited backbuffer to the `dst` texture
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            back_buf.as_ref(),
            Layout::TransferSrcOptimal,
            dst.as_ref(),
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
                extent: dims.as_extent(1),
            }),
        );

        // Optional step: Set the layout of dst when finsihed
        if let Some((layout, stage, access)) = self.dst_layout {
            dst.set_layout(&mut self.cmd_buf, layout, stage, access);
        }

        // Finish
        self.cmd_buf.finish();

        pool.driver().borrow_mut().get_queue_mut(QUEUE_TYPE).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(self.fence.as_ref()),
        );
    }

    unsafe fn write_descriptor_sets(&mut self) {
        let (mode, graphics) = self.graphics.as_ref().unwrap();
        let src = self.src.borrow();
        let dst = self.dst.borrow();
        let src_view = src.as_default_2d_view();

        if let Mode::Blend(_) = mode {
            let dst_view = dst.as_default_2d_view();
            self.pool
                .borrow()
                .driver()
                .borrow_mut()
                .write_descriptor_sets(
                    vec![
                        DescriptorSetWrite {
                            set: graphics.desc_set(0),
                            binding: 0,
                            array_offset: 0,
                            descriptors: once(Descriptor::CombinedImageSampler(
                                src_view.as_ref(),
                                Layout::ShaderReadOnlyOptimal,
                                graphics.sampler(0).as_ref(),
                            )),
                        },
                        DescriptorSetWrite {
                            set: graphics.desc_set(0),
                            binding: 1,
                            array_offset: 0,
                            descriptors: once(Descriptor::CombinedImageSampler(
                                dst_view.as_ref(),
                                Layout::ShaderReadOnlyOptimal,
                                graphics.sampler(0).as_ref(),
                            )),
                        },
                    ]
                    .drain(..),
                );
        } else {
            self.pool
                .borrow()
                .driver()
                .borrow_mut()
                .write_descriptor_sets(once(DescriptorSetWrite {
                    set: graphics.desc_set(0),
                    binding: 0,
                    array_offset: 0,
                    descriptors: once(Descriptor::CombinedImageSampler(
                        src_view.as_ref(),
                        Layout::ShaderReadOnlyOptimal,
                        graphics.sampler(0).as_ref(),
                    )),
                }));
        }
    }
}

struct WriteResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    op: WriteOp<S, D>,
}

impl<S, D> Drop for WriteResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<S, D> Op for WriteResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&*self.op.pool.borrow().driver().borrow(), &self.op.fence);
        }
    }
}
