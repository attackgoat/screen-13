use {
    super::{Device, DriverError, Image, ImageInfo, ImageType, SampleCount, Surface},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    log::{debug, warn},
    std::{ops::Deref, slice, thread::panicking, time::Duration},
};

#[derive(Debug)]
pub struct Swapchain<P>
where
    P: SharedPointerKind,
{
    device: SharedPointer<Device<P>, P>,
    images: Vec<Option<Image<P>>>,
    pub info: SwapchainInfo,
    next_semaphore: usize,
    acquired_semaphores: Vec<vk::Semaphore>,
    rendered_semaphores: Vec<vk::Semaphore>, // TODO: make a single semaphore
    suboptimal: bool,
    surface: Surface<P>,
    swapchain: vk::SwapchainKHR,
}

impl<P> Swapchain<P>
where
    P: SharedPointerKind,
{
    pub fn new(
        device: &SharedPointer<Device<P>, P>,
        surface: Surface<P>,
        info: SwapchainInfo,
    ) -> Result<Self, DriverError> {
        let device = SharedPointer::clone(device);
        let acquired_semaphores = (0..info.desired_image_count)
            .map(|_| {
                unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                    .unwrap()
            })
            .collect();

        let rendered_semaphores = (0..info.desired_image_count)
            .map(|_| {
                unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                    .unwrap()
            })
            .collect();

        Ok(Swapchain {
            device,
            images: vec![],
            info,
            next_semaphore: 0,
            acquired_semaphores,
            rendered_semaphores,
            suboptimal: true,
            surface,
            swapchain: vk::SwapchainKHR::null(),
        })
    }

    pub fn acquire_next_image(&mut self) -> Result<SwapchainImage<P>, SwapchainError> {
        if self.suboptimal {
            self.recreate_swapchain()
                .map_err(|_| SwapchainError::SurfaceLost)?;
            self.suboptimal = false;
        }

        let acquired = self.acquired_semaphores[self.next_semaphore];
        let rendered = self.rendered_semaphores[self.next_semaphore];
        let image_idx = unsafe {
            self.device
                .swapchain_ext
                .as_ref()
                .unwrap()
                .acquire_next_image(
                    self.swapchain,
                    Duration::from_secs_f32(10.0).as_nanos() as _,
                    acquired,
                    vk::Fence::null(),
                )
        }
        .map(|(idx, suboptimal)| {
            if suboptimal {
                self.suboptimal = true;
            }

            idx
        });

        match image_idx {
            Ok(image_idx) => {
                assert_eq!(image_idx as usize, self.next_semaphore);

                self.next_semaphore += 1;
                self.next_semaphore %= self.images.len();

                Ok(SwapchainImage {
                    image: self.images[image_idx as usize].take().unwrap(),
                    idx: image_idx,
                    acquired,
                    rendered,
                })
            }
            Err(err)
                if err == vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT
                    || err == vk::Result::ERROR_OUT_OF_DATE_KHR
                    || err == vk::Result::NOT_READY
                    || err == vk::Result::SUBOPTIMAL_KHR
                    || err == vk::Result::TIMEOUT =>
            {
                self.suboptimal = true;

                Err(SwapchainError::Suboptimal)
            }
            Err(err) if err == vk::Result::ERROR_DEVICE_LOST => {
                self.suboptimal = true;

                Err(SwapchainError::DeviceLost)
            }
            Err(err) if err == vk::Result::ERROR_SURFACE_LOST_KHR => {
                self.suboptimal = true;

                Err(SwapchainError::SurfaceLost)
            }
            _ => {
                // Probably:
                // VK_ERROR_OUT_OF_HOST_MEMORY
                // VK_ERROR_OUT_OF_DEVICE_MEMORY

                // TODO: Maybe handle timeout in here

                Err(SwapchainError::SurfaceLost)
            }
        }
    }

    fn destroy(&self) {
        if self.swapchain != vk::SwapchainKHR::null() {
            unsafe {
                self.device
                    .swapchain_ext
                    .as_ref()
                    .unwrap()
                    .destroy_swapchain(self.swapchain, None);
            }
        }
    }

    pub fn present_image(&mut self, image: SwapchainImage<P>) {
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(slice::from_ref(&image.rendered))
            .swapchains(slice::from_ref(&self.swapchain))
            .image_indices(slice::from_ref(&image.idx));

        unsafe {
            match self
                .device
                .swapchain_ext
                .as_ref()
                .unwrap()
                .queue_present(*self.device.queue, &present_info)
            {
                Ok(_) => (),
                Err(err)
                    if err == vk::Result::ERROR_DEVICE_LOST
                        || err == vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT
                        || err == vk::Result::ERROR_OUT_OF_DATE_KHR
                        || err == vk::Result::ERROR_SURFACE_LOST_KHR
                        || err == vk::Result::SUBOPTIMAL_KHR =>
                {
                    // Handled in the next frame
                    self.suboptimal = true;
                }
                _ => {
                    // Probably:
                    // VK_ERROR_OUT_OF_HOST_MEMORY
                    // VK_ERROR_OUT_OF_DEVICE_MEMORY
                    warn!("Unhandled error");
                }
            }
        }

        self.images[image.idx as usize] = Some(image.image);
    }

    fn recreate_swapchain(&mut self) -> Result<(), DriverError> {
        let res = unsafe { self.device.device_wait_idle() };

        if let Err(err) = res {
            warn!("device_wait_idle() failed: {err:?}");
        }

        self.destroy();

        let surface_ext = self.device.surface_ext.as_ref().unwrap();
        let surface_capabilities = unsafe {
            surface_ext.get_physical_device_surface_capabilities(
                *self.device.physical_device,
                *self.surface,
            )
        }
        .map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;

        // Triple-buffer so that acquiring an image doesn't stall for >16.6ms at 60Hz on AMD
        // when frames take >16.6ms to render. Also allows MAILBOX to work.
        let mut desired_image_count = self
            .info
            .desired_image_count
            .max(surface_capabilities.min_image_count);
        if surface_capabilities.max_image_count != 0 {
            desired_image_count = desired_image_count.min(surface_capabilities.max_image_count);
        }

        debug!("Swapchain image count: {}", desired_image_count);

        let (surface_width, surface_height) = match surface_capabilities.current_extent.width {
            std::u32::MAX => (
                // TODO: Maybe handle this case with aspect-correct clamping?
                self.info.width.clamp(
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.max_image_extent.width,
                ),
                self.info.height.clamp(
                    surface_capabilities.min_image_extent.height,
                    surface_capabilities.max_image_extent.height,
                ),
            ),
            _ => (
                surface_capabilities.current_extent.width,
                surface_capabilities.current_extent.height,
            ),
        };

        if surface_width * surface_height == 0 {
            return Err(DriverError::Unsupported);
        }

        let present_mode_preference = if self.info.sync_display {
            vec![vk::PresentModeKHR::FIFO_RELAXED, vk::PresentModeKHR::FIFO]
        } else {
            vec![vk::PresentModeKHR::MAILBOX, vk::PresentModeKHR::IMMEDIATE]
        };

        let present_modes = unsafe {
            surface_ext.get_physical_device_surface_present_modes(
                *self.device.physical_device,
                *self.surface,
            )
        }
        .map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;

        let present_mode = present_mode_preference
            .into_iter()
            .find(|mode| present_modes.contains(mode))
            .unwrap_or(vk::PresentModeKHR::FIFO);

        debug!("Presentation mode: {:?}", present_mode);

        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        // info!("supported_usage_flags {:#?}", &surface_capabilities.supported_usage_flags);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(*self.surface)
            .min_image_count(desired_image_count)
            .image_color_space(self.info.format.color_space)
            .image_format(self.info.format.format)
            .image_extent(vk::Extent2D {
                width: surface_width,
                height: surface_height,
            })
            .image_usage(surface_capabilities.supported_usage_flags)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1)
            .build();
        let swapchain_ext = self.device.swapchain_ext.as_ref().unwrap();
        let swapchain = unsafe { swapchain_ext.create_swapchain(&swapchain_create_info, None) }
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;

        let vk_images = unsafe { swapchain_ext.get_swapchain_images(swapchain) }.unwrap();
        let images: Vec<Option<Image<_>>> = vk_images
            .into_iter()
            .enumerate()
            .map(|(idx, vk_image)| {
                let mut image = Image::from_raw(
                    &self.device,
                    vk_image,
                    ImageInfo {
                        ty: ImageType::Texture2D,
                        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::STORAGE // TODO: Conditionally enable! When SRGB is supported for it!
                            | vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC,
                        flags: vk::ImageCreateFlags::empty(), // MUTABLE_FORMAT | SPARSE_ALIASED | CUBE_COMPATIBLE
                        fmt: vk::Format::B8G8R8A8_UNORM,      // TODO: Allow configuration!
                        depth: 0,                             // TODO: 1?
                        height: self.info.height,
                        width: self.info.width,
                        sample_count: SampleCount::X1,
                        linear_tiling: false,
                        mip_level_count: 1,
                        array_elements: 1,
                    },
                );
                image.name = Some(format!("swapchain{idx}"));
                Some(image)
            })
            .collect();

        assert_eq!(desired_image_count, images.len() as u32);

        self.next_semaphore = 0;
        self.images = images;
        self.swapchain = swapchain;

        Ok(())
    }
}

impl<P> Drop for Swapchain<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        for semaphore in self
            .acquired_semaphores
            .drain(..)
            .chain(self.rendered_semaphores.drain(..))
        {
            unsafe {
                self.device.destroy_semaphore(semaphore, None);
            }
        }

        self.destroy();
    }
}

#[derive(Debug)]
pub struct SwapchainImage<P>
where
    P: SharedPointerKind,
{
    pub acquired: vk::Semaphore,
    pub image: Image<P>,
    pub idx: u32,
    pub rendered: vk::Semaphore,
}

impl<P> Clone for SwapchainImage<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self {
            acquired: self.acquired,
            image: Image::clone_raw(&self.image),
            idx: self.idx,
            rendered: self.rendered,
        }
    }
}

impl<P> Deref for SwapchainImage<P>
where
    P: SharedPointerKind,
{
    type Target = Image<P>;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

#[derive(Debug)]
pub enum SwapchainError {
    /// This frame is lost but more may be acquired later.
    DeviceLost,
    /// This frame is not lost but there may be a delay while the next frame is recreated.
    Suboptimal,
    /// The surface was lost and must be recreated, which includes any operating system window.
    SurfaceLost,
}

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned")]
pub struct SwapchainInfo {
    pub desired_image_count: u32,
    pub format: vk::SurfaceFormatKHR,
    pub height: u32,
    pub sync_display: bool,
    pub width: u32,
}

impl SwapchainInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SwapchainInfoBuilder {
        Default::default()
    }
}
