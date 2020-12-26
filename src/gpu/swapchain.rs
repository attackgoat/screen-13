use {
    super::{
        def::{render_passes::present, Graphics},
        driver::{
            bind_graphics_descriptor_set, CommandPool, Fence, Framebuffer2d, RenderPass, Semaphore,
        },
        Device, Driver, Surface, Texture2d,
    },
    crate::math::{vec3, CoordF, Extent, Mat4},
    gfx_hal::{
        command::{ClearValue, CommandBuffer as _, CommandBufferFlags, Level, SubpassContents},
        device::Device as _,
        format::{ChannelType, Format},
        image::{Access, Layout},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags, Viewport},
        queue::{CommandQueue as _, Submission},
        window::{PresentationSurface as _, Surface as _, SurfaceCapabilities, SwapchainConfig},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        iter::{empty, once},
    },
};

// TODO: Test things while fiddling with the result of this function
fn pick_format(gpu: &<_Backend as Backend>::PhysicalDevice, surface: &Surface) -> Format {
    surface
        .supported_formats(gpu)
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
    driver: Driver,
    fmt: Format,
    graphics: Graphics,
    image_idx: usize,
    images: Vec<Image>,
    needs_configuration: bool,
    render_pass: RenderPass,
    supported_fmts: Vec<Format>,
    surface: Surface,
}

impl Swapchain {
    pub fn new(driver: &Driver, mut surface: Surface, dims: Extent, image_count: u32) -> Self {
        assert_ne!(image_count, 0);

        let mut needs_configuration = false;
        let (family, fmt, supported_fmts) = {
            let device = driver.as_ref().borrow();
            let family = Device::queue_family(&device);
            let gpu = Device::gpu(&device);
            let fmt = pick_format(gpu, &surface);
            let caps = surface.capabilities(gpu);
            let swap_config = swapchain_config(caps, dims, fmt, image_count);

            unsafe {
                surface
                    .configure_swapchain(&device, swap_config)
                    .unwrap_or_else(|_| needs_configuration = true);
            }

            let supported_fmts = surface.supported_formats(gpu).unwrap_or_default();

            (family, fmt, supported_fmts)
        };

        let desc_sets = 1;
        let render_pass = present(driver, fmt);
        let graphics = unsafe {
            Graphics::present(
                #[cfg(feature = "debug-names")]
                "Swapchain",
                driver,
                desc_sets,
                RenderPass::subpass(&render_pass, 0),
            )
        };

        let mut images = vec![];
        for _ in 0..image_count {
            let mut cmd_pool = CommandPool::new(driver, family);
            let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };
            images.push(Image {
                cmd_buf,
                cmd_pool,
                fence: Fence::with_signal(
                    #[cfg(feature = "debug-names")]
                    "Swapchain image",
                    driver,
                    true,
                ),
                signal: Semaphore::new(
                    #[cfg(feature = "debug-names")]
                    "Swapchain image",
                    driver,
                ),
            });
        }

        Self {
            dims,
            driver: Driver::clone(driver),
            fmt,
            graphics,
            images,
            image_idx: 0,
            needs_configuration,
            render_pass,
            supported_fmts,
            surface,
        }
    }

    fn configure(&mut self) {
        let device = self.driver.as_ref().borrow();
        let gpu = Device::gpu(&device);

        // Update the format as it may have changed
        self.fmt = pick_format(gpu, &self.surface);

        let caps = self.surface.capabilities(gpu);
        let swap_config = swapchain_config(caps, self.dims, self.fmt, self.images.len() as _);

        unsafe {
            if let Err(e) = self.surface.configure_swapchain(&device, swap_config) {
                warn!("Error configuring swapchain {:?}", e);

                self.needs_configuration = true;
            } else {
                self.needs_configuration = false;
            }
        }

        self.supported_fmts = self.surface.supported_formats(gpu).unwrap_or_default();
    }

    pub fn fmt(swapchain: &Self) -> Format {
        swapchain.fmt
    }

    pub fn present(&mut self, texture: &mut Texture2d) {
        if self.needs_configuration {
            debug!("Configuring swapchain");
            self.configure();

            if self.needs_configuration {
                info!("Unable to configure swapchain - not presenting");
                return;
            }
        }

        self.image_idx += 1;
        self.image_idx %= self.images.len();
        let image = &mut self.images[self.image_idx];

        let image_view = unsafe {
            // Allow 100ms for the next image to be ready
            match self.surface.acquire_image(100_000_000) {
                Err(_) => {
                    warn!("Unable to acquire swapchain image");
                    return;
                }
                Ok((image_view, suboptimal)) => {
                    // If it is suboptimal we will still present and configure on the next frame
                    if suboptimal.is_some() {
                        info!("Suboptimal swapchain image");
                        self.needs_configuration = true;
                    }

                    image_view
                }
            }
        };

        let frame_buf = Framebuffer2d::new(
            #[cfg(feature = "debug-names")]
            "Present",
            &self.driver,
            &self.render_pass,
            once(image_view.borrow()),
            self.dims,
        );

        let set = self.graphics.desc_set(0);
        let mut src = texture.borrow_mut();

        unsafe {
            let src_view = src.as_default_view();
            let device = self.driver.borrow_mut();
            let sampler = self.graphics.sampler(0).as_ref();
            device.write_descriptor_sets(once(DescriptorSetWrite {
                set,
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::CombinedImageSampler(
                    src_view.as_ref(),
                    Layout::ShaderReadOnlyOptimal,
                    sampler,
                )),
            }));
        }

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

        Fence::wait(&image.fence);
        Fence::reset(&mut image.fence);

        let mut device = self.driver.borrow_mut();
        let rect = self.dims.into();
        let viewport = Viewport {
            rect,
            depth: 0.0..1.0,
        };

        unsafe {
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
            bind_graphics_descriptor_set(&mut image.cmd_buf, self.graphics.layout(), set);
            image.cmd_buf.push_graphics_constants(
                self.graphics.layout(),
                ShaderStageFlags::VERTEX,
                0,
                VertexConsts { transform }.as_ref(),
            );
            image.cmd_buf.begin_render_pass(
                &self.render_pass,
                &frame_buf,
                rect,
                empty::<&ClearValue>(),
                SubpassContents::Inline,
            );
            image.cmd_buf.draw(0..6, 0..1);
            image.cmd_buf.end_render_pass();

            image.cmd_buf.finish();

            let queue = Device::queue_mut(&mut device);
            queue.submit(
                Submission {
                    command_buffers: once(&image.cmd_buf),
                    wait_semaphores: empty(),
                    signal_semaphores: once(image.signal.as_ref()),
                },
                Some(&image.fence),
            );
            match queue.present(&mut self.surface, image_view, Some(&image.signal)) {
                Err(e) => {
                    warn!("Unable to present swapchain image");
                    self.needs_configuration = true;
                    Err(e)
                }
                Ok(_suboptimal) => {
                    // TODO: Learn more about this
                    // if suboptimal.is_some() {
                    //     info!("Suboptimal swapchain");
                    //     //self.needs_configuration = true;
                    // }

                    Ok(())
                }
            }
            .unwrap_or_default();
        }
    }

    pub fn supported_formats(&self) -> &[Format] {
        &self.supported_fmts
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();

        unsafe {
            self.surface.unconfigure_swapchain(&device);
        }
    }
}

#[repr(C)]
struct VertexConsts {
    transform: Mat4,
}

impl AsRef<[u32; 16]> for VertexConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 16] {
        unsafe { &*(self as *const Self as *const [u32; 16]) }
    }
}
