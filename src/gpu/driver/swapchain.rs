use {
    super::{Driver, PhysicalDevice, Surface},
    crate::math::Extent,
    gfx_hal::{
        device::Device,
        format::{ChannelType, Format},
        image::Usage,
        window::{PresentMode, Surface as _, SurfaceCapabilities, SwapchainConfig},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

// TODO: Test things while fiddling with the result of this function
fn pick_format(gpu: &<_Backend as Backend>::PhysicalDevice, surface: &Surface) -> Format {
    surface
        .supported_formats(gpu)
        .map_or(Format::Bgra8Srgb, |formats| {
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
    // All presentaion will happen by copying intermediately rendered images into the surface images
    let mut swap_config = SwapchainConfig::from_caps(&caps, format, dims.into())
        .with_image_usage(Usage::COLOR_ATTACHMENT);

    // We want one image more than the minimum but still less than or equal to the maxium
    swap_config.image_count = image_count
        .max(*caps.image_count.start())
        .min(*caps.image_count.end());

    // We want triple buffering if available
    if caps.present_modes.contains(PresentMode::MAILBOX) {
        swap_config.present_mode = PresentMode::MAILBOX;
    }

    swap_config.image_usage |= Usage::TRANSFER_DST;

    swap_config
}

#[derive(Debug)]
pub struct Frame {
    image: <_Backend as Backend>::Image,
}

#[derive(Debug)]
pub struct Swapchain {
    driver: Driver,
    format: Format,
    image_count: u32,
    supported_formats: Vec<Format>,
    surface: Surface,
    swapchain: Option<<_Backend as Backend>::Swapchain>,
}

impl Swapchain {
    pub fn from_surface(
        mut surface: Surface,
        driver: Driver,
        dims: Extent,
        image_count: u32,
    ) -> (Self, Vec<<_Backend as Backend>::Image>) {
        let (backbuffer_images, format, supported_formats, swapchain) = {
            let device = driver.borrow();
            let gpu = device.gpu();
            let format = pick_format(gpu, &surface);
            let caps = surface.capabilities(gpu);
            let swap_config = swapchain_config(caps, dims, format, image_count);
            let (swapchain, backbuffer_images) = unsafe {
                device
                    .create_swapchain(&mut surface, swap_config, None)
                    .unwrap()
            };
            let supported_formats = surface.supported_formats(gpu).unwrap_or_default();
            (backbuffer_images, format, supported_formats, swapchain)
        };

        (
            Self {
                driver,
                format,
                image_count,
                supported_formats,
                surface,
                swapchain: Some(swapchain),
            },
            backbuffer_images,
        )
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn recreate(&mut self, dims: Extent) -> Vec<<_Backend as Backend>::Image> {
        let device = self.driver.borrow();
        let gpu = device.gpu();
        self.format = pick_format(gpu, &self.surface);

        let caps = self.surface.capabilities(gpu);
        let swap_config = swapchain_config(caps, dims, self.format, self.image_count);
        let (swapchain, backbuffer_images) = unsafe {
            device
                .create_swapchain(
                    &mut self.surface,
                    swap_config,
                    Some(self.swapchain.take().unwrap()),
                )
                .unwrap()
        };

        self.swapchain.replace(swapchain);
        self.supported_formats = self.surface.supported_formats(gpu).unwrap_or_default();

        backbuffer_images
    }

    pub fn supported_formats(&self) -> &[Format] {
        &self.supported_formats
    }
}

impl Deref for Swapchain {
    type Target = <_Backend as Backend>::Swapchain;

    fn deref(&self) -> &Self::Target {
        self.swapchain.as_ref().unwrap()
    }
}

impl DerefMut for Swapchain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.swapchain.as_mut().unwrap()
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        if let Some(swapchain) = self.swapchain.take() {
            unsafe {
                self.driver.borrow().destroy_swapchain(swapchain);
            }
        }
    }
}
