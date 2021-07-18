use {
    super::{
        adapter,
        def::{push_const::Mat4PushConst, render_pass::present, Graphics},
        device,
        driver::{
            bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d, RenderPass, Semaphore,
        },
        queue_family, queue_mut, Surface, Texture2d,
    },
    crate::math::{vec3, CoordF, Extent, Mat4},
    gfx_hal::{
        command::{
            CommandBuffer as _, CommandBufferFlags, Level, RenderAttachmentInfo, SubpassContents,
        },
        device::Device as _,
        format::{ChannelType, Format},
        image::{Access, FramebufferAttachment, Layout, Usage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::Queue as _,
        window::{PresentMode, PresentationSurface as _, Surface as _, SwapchainConfig},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        iter::{empty, once},
    },
};

// TODO: Test things while fiddling with the result of this function
unsafe fn pick_format(surface: &Surface) -> Format {
    surface
        .supported_formats(&adapter().physical_device)
        .map_or(Format::Rgba8Srgb, |formats| {
            *formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .unwrap_or(&formats[0])
        })
}

struct Image {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: CommandPool,
    fence: Fence,
    signal: Semaphore,
}

pub enum PresentError {
    OutOfMemory,
    SurfaceLost,
}

pub struct Swapchain {
    dims: Extent,
    fmt: Format,
    frame_buf: Option<Framebuffer2d>,
    frame_buf_attachment: Option<FramebufferAttachment>,
    graphics: Graphics,
    image_idx: usize,
    images: Vec<Image>,
    render_pass: RenderPass,
    supported_fmts: Vec<Format>,
    surface: Surface,
    v_sync: bool,
}

impl Swapchain {
    pub(super) unsafe fn new(
        surface: Surface,
        dims: Extent,
        image_count: u32,
        v_sync: bool,
    ) -> Self {
        assert_ne!(image_count, 0);

        let fmt = pick_format(&surface);
        let supported_fmts = surface
            .supported_formats(&adapter().physical_device)
            .unwrap_or_default();

        let render_pass = present(fmt);
        let subpass = RenderPass::subpass(&render_pass, 0);
        let graphics = Graphics::present(
            #[cfg(feature = "debug-names")]
            "Swapchain",
            subpass,
            image_count as _,
        );

        let mut images = vec![];
        for _ in 0..image_count {
            let mut cmd_pool = CommandPool::new(queue_family());
            let cmd_buf = cmd_pool.allocate_one(Level::Primary);
            images.push(Image {
                cmd_buf,
                cmd_pool,
                fence: Fence::new_signal(
                    #[cfg(feature = "debug-names")]
                    "Swapchain image",
                    true,
                ),
                signal: Semaphore::new(
                    #[cfg(feature = "debug-names")]
                    "Swapchain image",
                ),
            });
        }

        Self {
            dims,
            fmt,
            frame_buf: None,
            frame_buf_attachment: None,
            graphics,
            images,
            image_idx: 0,
            render_pass,
            supported_fmts,
            surface,
            v_sync,
        }
    }

    unsafe fn configure(&mut self) {
        // Make sure all images have finished presentation first
        for image in &self.images {
            Fence::wait(&image.fence);
        }

        // Update the format as it may have changed
        self.fmt = pick_format(&self.surface);

        let caps = self.surface.capabilities(&adapter().physical_device);
        let swap_config = SwapchainConfig::from_caps(&caps, self.fmt, self.dims.into())
            .with_image_count(
                (self.images.len() as u32)
                    .max(*caps.image_count.start())
                    .min(*caps.image_count.end()),
            )
            .with_image_usage(
                Usage::COLOR_ATTACHMENT
                    | Usage::SAMPLED
                    | Usage::TRANSFER_DST
                    | Usage::TRANSFER_SRC,
            )
            .with_present_mode(
                if self.v_sync && caps.present_modes.contains(PresentMode::MAILBOX) {
                    PresentMode::MAILBOX
                } else if self.v_sync && caps.present_modes.contains(PresentMode::FIFO) {
                    PresentMode::FIFO
                } else {
                    PresentMode::IMMEDIATE
                },
            );
        let frame_buf_attachment = swap_config.framebuffer_attachment();
        if let Err(e) = self.surface.configure_swapchain(device(), swap_config) {
            warn!("Error configuring swapchain {:?}", e);

            // We need configuration!
            self.frame_buf_attachment = None;
        } else {
            self.frame_buf_attachment = Some(frame_buf_attachment);
            self.frame_buf = Some(Framebuffer2d::new(
                #[cfg(feature = "debug-names")]
                "Present",
                &self.render_pass,
                once(self.frame_buf_attachment.as_ref().unwrap().clone()),
                self.dims,
            ));
        }

        self.supported_fmts = self
            .surface
            .supported_formats(&adapter().physical_device)
            .unwrap_or_default();
    }

    pub fn dims(&self) -> Extent {
        self.dims
    }

    pub fn fmt(swapchain: &Self) -> Format {
        swapchain.fmt
    }

    pub unsafe fn present(&mut self, src: &Texture2d) {
        trace!("present image #{}", self.image_idx);

        // We must have a frame buffer attachment (a configured swapchain) in order to present
        if self.frame_buf_attachment.is_none() {
            debug!("Configuring swapchain");
            self.configure();

            if self.frame_buf_attachment.is_none() {
                // TODO: Warn? Or a helpful comment....
                info!("Unable to configure swapchain - not presenting");
                return;
            }
        }

        // Allow 100ms for the next image to be ready
        let image_view = match self.surface.acquire_image(100_000_000) {
            Err(_) => {
                warn!("Unable to acquire swapchain image");
                return;
            }
            Ok((image_view, suboptimal)) => {
                // If it is suboptimal we will still present and configure on the next frame
                if suboptimal.is_some() {
                    info!("Suboptimal swapchain image");
                    self.frame_buf_attachment = None;
                }

                image_view
            }
        };

        let image = &mut self.images[self.image_idx];
        Fence::wait(&image.fence);
        Fence::reset(&mut image.fence);
        image.cmd_pool.reset(false);
        Self::write_descriptor(&mut self.graphics, self.image_idx, src);

        let transform = {
            let dst_dims: CoordF = self.dims.into();
            let src_dims: CoordF = src.dims().into();
            let scale_x = dst_dims.x / src_dims.x;
            let scale_y = dst_dims.y / src_dims.y;
            let scale = scale_x.max(scale_y);
            Mat4::from_scale(vec3(
                src_dims.x * scale / dst_dims.x,
                src_dims.y * scale / dst_dims.y,
                1.0,
            ))
        };
        let rect = self.dims.into();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        image
            .cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
        src.set_layout(
            &mut image.cmd_buf,
            Layout::ShaderReadOnlyOptimal,
            PipelineStage::FRAGMENT_SHADER,
            Access::SHADER_READ,
        );
        image.cmd_buf.set_scissors(0, once(rect));
        image.cmd_buf.set_viewports(0, once(viewport));
        image
            .cmd_buf
            .bind_graphics_pipeline(self.graphics.pipeline());
        bind_graphics_descriptor_set(
            &mut image.cmd_buf,
            self.graphics.layout(),
            self.graphics.desc_set(self.image_idx),
        );
        image.cmd_buf.push_graphics_constants(
            self.graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            Mat4PushConst { val: transform }.as_ref(),
        );
        image.cmd_buf.begin_render_pass(
            &self.render_pass,
            self.frame_buf.as_ref().unwrap(),
            rect,
            once(RenderAttachmentInfo {
                image_view: image_view.borrow(),
                clear_value: Default::default(),
            }),
            SubpassContents::Inline,
        );
        image.cmd_buf.draw(0..6, 0..1);
        image.cmd_buf.end_render_pass();

        image.cmd_buf.finish();

        let queue = queue_mut();
        queue.submit(
            once(&image.cmd_buf),
            empty(),
            once(image.signal.as_ref()),
            Some(&mut image.fence),
        );
        match queue.present(&mut self.surface, image_view, Some(&mut image.signal)) {
            Err(e) => {
                warn!("Unable to present swapchain image");
                self.frame_buf_attachment = None;
                Err(e)
            }
            Ok(_suboptimal) => {
                // TODO: Learn more about this
                // if suboptimal.is_some() {
                //     info!("Suboptimal swapchain");
                //     //self.frame_buf_attachment = None;
                // }

                Ok(())
            }
        }
        .unwrap_or_default();

        // Advance to the next swapchain image
        self.image_idx += 1;
        self.image_idx %= self.images.len();
    }

    pub fn set_dims(&mut self, dims: Extent) {
        self.frame_buf_attachment = None;
        self.dims = dims;
    }

    pub fn supported_formats(&self) -> &[Format] {
        &self.supported_fmts
    }

    unsafe fn write_descriptor(graphics: &mut Graphics, idx: usize, texture: &Texture2d) {
        let (set, samplers) = graphics.desc_set_mut_with_samplers(idx);
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
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.surface.unconfigure_swapchain(device());
        }
    }
}
