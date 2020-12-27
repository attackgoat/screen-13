use {
    super::Op,
    crate::{
        color::AlphaColor,
        gpu::{
            def::{ColorRenderPassMode, Graphics, GraphicsMode, RenderPassMode},
            driver::{
                bind_graphics_descriptor_set, CommandPool, Device, Driver, Fence, Framebuffer2d,
            },
            pool::{Lease, Pool},
            Texture2d,
        },
        math::Coord,
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level, SubpassContents},
        format::Aspects,
        image::{Access as ImageAccess, Layout, Offset, SubresourceLayers, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{PipelineStage, Rect, Viewport},
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        iter::{empty, once},
    },
};

type Path = [(Coord, AlphaColor); 2];

fn graphics_mode(preserve_dst: bool) -> GraphicsMode {
    if preserve_dst {
        GraphicsMode::Gradient(true)
    } else {
        GraphicsMode::Gradient(false)
    }
}

fn must_preserve_dst(path: &Path) -> bool {
    path[0].1.is_transparent() || path[1].1.is_transparent()
}

pub struct GradientOp {
    back_buf: Lease<Texture2d>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: Texture2d,
    dst_preserve: bool,
    fence: Lease<Fence>,
    frame_buf: Framebuffer2d,
    graphics: Lease<Graphics>,
    pool: Option<Lease<Pool>>,
    path: [(Coord, AlphaColor); 2],
}

impl GradientOp {
    /// # Safety
    /// None
    #[must_use]
    pub fn new(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        mut pool: Lease<Pool>,
        dst: &Texture2d,
        path: Path,
    ) -> Self {
        // Allocate the command buffer
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(driver, family);

        let (dims, fmt) = {
            let dst = dst.borrow();
            (dst.dims(), dst.format())
        };

        let render_pass_mode = RenderPassMode::Color(ColorRenderPassMode {
            fmt,
            preserve: must_preserve_dst(&path),
        });

        // Setup the first pass graphics pipeline
        let graphics = pool.graphics(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            render_pass_mode,
            0,
            GraphicsMode::Gradient(false),
        );

        // Setup the framebuffer
        let back_buf = pool.texture(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            dims,
            fmt,
            Layout::Undefined,
            ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
            1,
            1,
            1,
        );
        let frame_buf = Framebuffer2d::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            pool.render_pass(&driver, render_pass_mode),
            once(back_buf.borrow().as_default_view().as_ref()),
            dims,
        );
        let fence = pool.fence(
            #[cfg(feature = "debug-names")]
            name,
            &driver,
        );

        Self {
            back_buf,
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(driver),
            dst: Texture2d::clone(dst),
            dst_preserve: false,
            fence,
            frame_buf,
            graphics,
            pool: Some(pool),
            path,
        }
    }

    /// Preserves the contents of the destination texture. Without calling this function the existing
    /// contents of the destination texture will not be composited into the final result.
    #[must_use]
    pub fn with_preserve(&mut self, val: bool) -> &mut Self {
        self.dst_preserve = val;
        self
    }

    pub fn record(&mut self) {
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
        }
    }

    unsafe fn submit(&mut self) {
        trace!("submit");

        let mut device = self.driver.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let mut back_buf = self.back_buf.borrow_mut();
        let preserve = self.dst_preserve && must_preserve_dst(&self.path);
        let _mode = RenderPassMode::Color(ColorRenderPassMode {
            fmt: dst.format(),
            preserve,
        });
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
        if preserve {
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
                    extent: dims.as_extent_depth(1),
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
        if preserve {
            dst.set_layout(
                &mut self.cmd_buf,
                Layout::ShaderReadOnlyOptimal,
                PipelineStage::FRAGMENT_SHADER,
                ImageAccess::SHADER_READ,
            );
        }

        // self.cmd_buf.begin_render_pass(
        //     pool.render_pass(mode),
        //     self.frame_buf.as_ref(),
        //     rect,
        //     vec![&TRANSPARENT_BLACK.into()].drain(..),
        //     SubpassContents::Inline,
        // );
        // TEMP
        let _ = SubpassContents::Inline;
        // TEMP

        self.cmd_buf
            .bind_graphics_pipeline(self.graphics.pipeline());
        if preserve {
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
                extent: dims.as_extent_depth(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        Device::queue_mut(&mut device).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }
}

impl Drop for GradientOp {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for GradientOp {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn take_pool(&mut self) -> Option<Lease<Pool>> {
        self.pool.take()
    }

    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
