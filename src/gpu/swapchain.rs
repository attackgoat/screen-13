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
        image::{Access, FramebufferAttachment, Layout},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
        window::{PresentationSurface as _, Surface as _, SurfaceCapabilities, SwapchainConfig},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
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

fn swapchain_config(
    caps: SurfaceCapabilities,
    dims: Extent,
    format: Format,
    image_count: u32,
) -> SwapchainConfig {
    let image_count = image_count
        .max(*caps.image_count.start())
        .min(*caps.image_count.end());

    SwapchainConfig::from_caps(&caps, format, dims.into()).with_image_count(image_count)
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
    frame_buf_attachment: Option<FramebufferAttachment>,
    graphics: Graphics,
    image_idx: usize,
    images: Vec<Image>,
    render_pass: RenderPass,
    supported_fmts: Vec<Format>,
    surface: Surface,
}

impl Swapchain {
    pub(super) unsafe fn new(mut surface: Surface, dims: Extent, image_count: u32) -> Self {
        assert_ne!(image_count, 0);

        let mut needs_configuration = false;
        let fmt = pick_format(&surface);
        let caps = surface.capabilities(&adapter().physical_device);
        let swap_config = swapchain_config(caps, dims, fmt, image_count);
        surface
            .configure_swapchain(device(), swap_config)
            .unwrap_or_else(|_| needs_configuration = true);

        let supported_fmts = surface
            .supported_formats(&adapter().physical_device)
            .unwrap_or_default();

        let desc_sets = 1;
        let render_pass = present(fmt);
        let subpass = RenderPass::subpass(&render_pass, 0);
        let graphics = Graphics::present(
            #[cfg(feature = "debug-names")]
            "Swapchain",
            subpass,
            desc_sets,
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
            frame_buf_attachment: None,
            graphics,
            images,
            image_idx: 0,
            render_pass,
            supported_fmts,
            surface,
        }
    }

    unsafe fn configure(&mut self) {
        // Update the format as it may have changed
        self.fmt = pick_format(&self.surface);

        let caps = self.surface.capabilities(&adapter().physical_device);
        let swap_config = swapchain_config(caps, self.dims, self.fmt, self.images.len() as _);
        let frame_buf_attachment = swap_config.framebuffer_attachment();
        if let Err(e) = self.surface.configure_swapchain(device(), swap_config) {
            warn!("Error configuring swapchain {:?}", e);

            // We need configuration!
            self.frame_buf_attachment = None;
        } else {
            self.frame_buf_attachment = Some(frame_buf_attachment);
        }

        self.supported_fmts = self
            .surface
            .supported_formats(&adapter().physical_device)
            .unwrap_or_default();
    }

    pub fn fmt(swapchain: &Self) -> Format {
        swapchain.fmt
    }

    pub unsafe fn present(&mut self, texture: &mut Texture2d) {
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

        let frame_buf = Framebuffer2d::new(
            #[cfg(feature = "debug-names")]
            "Present",
            &self.render_pass,
            once(self.frame_buf_attachment.as_ref().unwrap().clone()),
            self.dims,
        );

        self.write_descriptor(texture);

        let mut src = texture.borrow_mut();
        let dst_dims: CoordF = self.dims.into();
        let src_dims: CoordF = src.dims().into();

        // Scale is the larger of either X or Y when stretching to cover all four sides
        let scale_x = dst_dims.x / src_dims.x;
        let scale_y = dst_dims.y / src_dims.y;
        let scale = scale_x.max(scale_y);

        // Transform is scaled and centered on the dst texture
        let transform = Mat4::from_scale(vec3(
            src_dims.x * scale / dst_dims.x * 2.0,
            src_dims.y * scale / dst_dims.y * 2.0,
            1.0,
        )) * Mat4::from_translation(vec3(-0.5, -0.5, 0.0));

        self.image_idx += 1;
        self.image_idx %= self.images.len();
        let image = &mut self.images[self.image_idx];
        Fence::wait(&image.fence);
        Fence::reset(&mut image.fence);

        let rect = self.dims.into();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        image.cmd_pool.reset(false);

        image
            .cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        src.set_layout(
            &mut image.cmd_buf,
            Layout::ShaderReadOnlyOptimal,
            PipelineStage::FRAGMENT_SHADER,
            Access::SHADER_READ,
        );

        image.cmd_buf.set_scissors(0, &[rect]);
        image.cmd_buf.set_viewports(0, &[viewport]);
        image
            .cmd_buf
            .bind_graphics_pipeline(self.graphics.pipeline());
        bind_graphics_descriptor_set(
            &mut image.cmd_buf,
            self.graphics.layout(),
            self.graphics.desc_set(0),
        );
        image.cmd_buf.push_graphics_constants(
            self.graphics.layout(),
            ShaderStageFlags::VERTEX,
            0,
            Mat4PushConst { val: transform }.as_ref(),
        );
        image.cmd_buf.begin_render_pass(
            &self.render_pass,
            &frame_buf,
            rect,
            empty::<RenderAttachmentInfo<_Backend>>(),
            SubpassContents::Inline,
        );
        image.cmd_buf.draw(0..6, 0..1);
        image.cmd_buf.end_render_pass();

        image.cmd_buf.finish();

        let queue = queue_mut();
        queue.submit(
            Submission {
                command_buffers: once(&image.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: once(image.signal.as_ref()),
            },
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
    }

    pub fn supported_formats(&self) -> &[Format] {
        &self.supported_fmts
    }

    unsafe fn write_descriptor(&mut self, texture: &Texture2d) {
        let (set, samplers) = self.graphics.desc_set_mut_with_samplers(0);
        device().write_descriptor_set(DescriptorSetWrite {
            set,
            binding: 0,
            array_offset: 0,
            descriptors: once(Descriptor::CombinedImageSampler(
                texture.borrow().as_2d_color().as_ref(),
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
