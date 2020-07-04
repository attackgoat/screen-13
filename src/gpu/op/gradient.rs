use {
    super::{wait_for_fence, Op},
    crate::{
        color::{AlphaColor, TRANSPARENT_BLACK},
        gpu::{
            driver::{
                bind_graphics_descriptor_set, CommandPool, Driver, Fence, Framebuffer2d, Image2d,
                PhysicalDevice,
            },
            pool::{Graphics, GraphicsMode, Lease, RenderPassMode},
            PoolRef, TextureRef,
        },
        math::Coord,
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        format::Aspects,
        image::{
            Access as ImageAccess, Layout, Offset, SubresourceLayers, Tiling, Usage as ImageUsage,
        },
        pool::CommandPool as _,
        pso::{PipelineStage, Rect, Viewport},
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

type Path = [(Coord, AlphaColor); 2];

const QUEUE_TYPE: QueueType = QueueType::Graphics;

fn graphics_mode(preserve_dst: bool) -> GraphicsMode {
    if preserve_dst {
        GraphicsMode::GradientTransparency
    } else {
        GraphicsMode::Gradient
    }
}

fn must_preserve_dst(path: &Path) -> bool {
    path[0].1.is_transparent() || path[1].1.is_transparent()
}

fn render_pass_mode(preserve_dst: bool) -> RenderPassMode {
    if preserve_dst {
        RenderPassMode::ReadWrite
    } else {
        RenderPassMode::Write
    }
}

pub struct GradientOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    back_buf: Lease<TextureRef<Image2d>>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dst: TextureRef<I>,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics: Lease<Graphics>,
    path: [(Coord, AlphaColor); 2],
    pool: PoolRef,
}

impl<I> GradientOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    /// # Safety
    /// None
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        dst: &TextureRef<I>,
        path: Path,
    ) -> Self {
        let mut pool_ref = pool.borrow_mut();
        let driver = Driver::clone(pool_ref.driver());

        // Allocate the command buffer
        let family = driver.borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);

        // Setup the first pass graphics pipeline
        let graphics = pool_ref.graphics(
            #[cfg(debug_assertions)]
            name,
            GraphicsMode::Gradient,
            RenderPassMode::ReadWrite,
            0,
        );

        let (dims, format) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        // Setup the framebuffer
        let back_buf = pool_ref.texture(
            #[cfg(debug_assertions)]
            name,
            dims,
            Tiling::Optimal,
            format,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::INPUT_ATTACHMENT
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );
        let mode = render_pass_mode(must_preserve_dst(&path));
        let frame_buf = Framebuffer2d::new(
            Driver::clone(&driver),
            pool_ref.render_pass(mode),
            once(back_buf.borrow().as_default_2d_view().as_ref()),
            dims,
        );

        Self {
            back_buf,
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            dst: TextureRef::clone(dst),
            fence: pool_ref.fence(),
            frame_buf,
            graphics,
            path,
            pool: PoolRef::clone(pool),
        }
    }

    pub fn record(mut self) -> GradientResult<I> {
        // Setup the descriptor set
        {
            // let pool = self.pool.borrow();
            // let device = pool.driver().borrow();

            // let colors = self.gbuf[0].borrow();
            // let positions = self.gbuf[1].borrow();
            // let normals = self.gbuf[2].borrow();
            // let materials = self.gbuf[3].borrow();
            // let depths = self.gbuf[4].borrow();

            // let colors_view = colors.as_default_2d_view();
            // let positions_view = positions.as_default_2d_view();
            // let normals_view = normals.as_default_2d_view();
            // let materials_view = materials.as_default_2d_view();
            // let depths_view = depths.as_view(
            //     ViewKind::D2,
            //     Format::D32Sfloat,
            //     Default::default(),
            //     SubresourceRange {
            //         aspects: Aspects::DEPTH,
            //         levels: 0..1,
            //         layers: 0..1,
            //     },
            // );

            // unsafe {
            //     device.write_descriptor_sets(once(DescriptorSetWrite {
            //         set: self.sunlight.desc_set(0),
            //         binding: 0,
            //         array_offset: 0,
            //         descriptors: &[
            //             Descriptor::CombinedImageSampler(
            //                 colors_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.sunlight.sampler(0).as_ref(),
            //             ),
            //             Descriptor::CombinedImageSampler(
            //                 positions_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.sunlight.sampler(1).as_ref(),
            //             ),
            //             Descriptor::CombinedImageSampler(
            //                 normals_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.sunlight.sampler(2).as_ref(),
            //             ),
            //             Descriptor::CombinedImageSampler(
            //                 materials_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.sunlight.sampler(3).as_ref(),
            //             ),
            //             Descriptor::CombinedImageSampler(
            //                 depths_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.sunlight.sampler(4).as_ref(),
            //             ),
            //         ],
            //     }));

            //     device.write_descriptor_sets(once(DescriptorSetWrite {
            //         set: self.trans.desc_set(0),
            //         binding: 0,
            //         array_offset: 0,
            //         descriptors: &[
            //             Descriptor::CombinedImageSampler(
            //                 colors_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.trans.sampler(0).as_ref(),
            //             ),
            //             Descriptor::CombinedImageSampler(
            //                 depths_view.as_ref(),
            //                 Layout::ShaderReadOnlyOptimal,
            //                 self.trans.sampler(1).as_ref(),
            //             ),
            //         ],
            //     }));
            //}
        }

        unsafe {
            self.submit();
        };

        GradientResult {
            cmd_pool: self.cmd_pool,
            driver: Driver::clone(self.pool.borrow().driver()),
            dst: self.dst,
            fence: self.fence,
        }
    }

    unsafe fn submit(&mut self) {
        let mut pool = self.pool.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let mut back_buf = self.back_buf.borrow_mut();
        let preserve_dst = must_preserve_dst(&self.path);
        let mode = render_pass_mode(preserve_dst);
        let dims = dst.dims();
        let rect = Rect {
            x: 0,
            y: 0,
            w: dims.x as _,
            h: dims.y as _,
        };
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Optional step: Fill dst into the color graphics buffer in order to preserve it in the output
        if preserve_dst {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::TransferSrcOptimal,
                PipelineStage::TRANSFER,
                ImageAccess::TRANSFER_READ,
            );
            back_buf.set_layout(
                &mut self.cmd_buf,
                Layout::TransferDstOptimal,
                PipelineStage::TRANSFER,
                ImageAccess::TRANSFER_WRITE,
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
        }

        // Step 1: Draw the gradient, optionally providing `dst`
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::ColorAttachmentOptimal,
            PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            ImageAccess::COLOR_ATTACHMENT_WRITE,
        );
        if preserve_dst {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::ShaderReadOnlyOptimal,
                PipelineStage::FRAGMENT_SHADER,
                ImageAccess::SHADER_READ,
            );
        }
        self.cmd_buf.begin_render_pass(
            pool.render_pass(mode),
            self.frame_buf.as_ref(),
            rect,
            vec![&TRANSPARENT_BLACK.into()].drain(..),
            SubpassContents::Inline,
        );
        self.cmd_buf
            .bind_graphics_pipeline(self.graphics.pipeline());
        if preserve_dst {
            bind_graphics_descriptor_set(
                &mut self.cmd_buf,
                self.graphics.layout(),
                self.graphics.desc_set(0),
            );
        }
        self.cmd_buf.set_scissors(0, &[rect]);
        self.cmd_buf.set_viewports(0, &[viewport]);
        self.cmd_buf.draw(0..6, 0..1);
        self.cmd_buf.end_render_pass();

        // Step 2: Copy the now-composited backbuffer to the `dst` texture
        back_buf.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
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

        // Finish
        self.cmd_buf.finish();

        // Submit
        pool.driver().borrow_mut().get_queue_mut(QUEUE_TYPE).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(self.fence.as_ref()),
        );
    }
}

pub struct GradientResult<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: TextureRef<I>,
    fence: Lease<Fence>,
}

impl<I> Drop for GradientResult<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<I> Op for GradientResult<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&self.driver.borrow(), &self.fence);
        }
    }
}
