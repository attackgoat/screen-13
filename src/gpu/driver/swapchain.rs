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
    fmt: Format,
    image_count: u32,
    ptr: Option<<_Backend as Backend>::Swapchain>,
    supported_fmts: Vec<Format>,
    surface: Surface,
}

impl Swapchain {
    pub fn new(
        driver: Driver,
        mut surface: Surface,
        dims: Extent,
        image_count: u32,
    ) -> (Self, Vec<<_Backend as Backend>::Image>) {
        let (backbuffer_images, fmt, supported_fmts, swapchain) = {
            let device = driver.borrow();
            let gpu = device.gpu();
            let fmt = pick_format(gpu, &surface);
            let caps = surface.capabilities(gpu);
            let swap_config = swapchain_config(caps, dims, fmt, image_count);
            let (swapchain, backbuffer_images) =
                unsafe { device.create_swapchain(&mut surface, swap_config, None) }.unwrap();
            let supported_fmts = surface.supported_formats(gpu).unwrap_or_default();
            (backbuffer_images, fmt, supported_fmts, swapchain)
        };

        (
            Self {
                driver,
                fmt,
                image_count,
                ptr: Some(swapchain),
                supported_fmts,
                surface,
            },
            backbuffer_images,
        )
    }

    pub fn fmt(swapchain: &Self) -> Format {
        swapchain.fmt
    }

    pub fn recreate(swapchain: &mut Self, dims: Extent) -> Vec<<_Backend as Backend>::Image> {
        let device = swapchain.driver.borrow();
        let gpu = device.gpu();

        // Update the format as it may have changed
        swapchain.fmt = pick_format(gpu, &swapchain.surface);

        let caps = swapchain.surface.capabilities(gpu);
        let swap_config = swapchain_config(caps, dims, swapchain.fmt, swapchain.image_count);
        let (new_ptr, backbuffer_images) = {
            let old_ptr = swapchain.ptr.take().unwrap();

            unsafe {
                device
                    .create_swapchain(&mut swapchain.surface, swap_config, Some(old_ptr))
                    .unwrap() // TODO: Handle this error!
            }
        };

        swapchain.ptr.replace(new_ptr);
        swapchain.supported_fmts = swapchain.surface.supported_formats(gpu).unwrap_or_default();

        backbuffer_images
    }

    pub fn supported_formats(self: &Self) -> &[Format] {
        &self.supported_fmts
    }
}

impl AsMut<<_Backend as Backend>::Swapchain> for Swapchain {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Swapchain {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Swapchain> for Swapchain {
    fn as_ref(&self) -> &<_Backend as Backend>::Swapchain {
        &*self
    }
}

impl Deref for Swapchain {
    type Target = <_Backend as Backend>::Swapchain;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Swapchain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            let device = self.driver.borrow();

            unsafe {
                device.destroy_swapchain(ptr);
            }
        }
    }
}
