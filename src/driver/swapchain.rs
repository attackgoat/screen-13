use {
    super::{Device, DriverError, Image, ImageInfo, ImageType, SampleCount, Surface},
    ash::vk,
    log::{debug, warn},
    std::{ops::Deref, slice, sync::Arc, thread::panicking, time::Duration},
};

#[derive(Debug)]
pub struct Swapchain {
    device: Arc<Device>,
    images: Vec<Option<Image>>,
    pub info: SwapchainInfo,
    next_semaphore: usize,
    acquired_semaphores: Vec<vk::Semaphore>,
    rendered_semaphores: Vec<vk::Semaphore>, // TODO: make a single semaphore
    suboptimal: bool,
    surface: Surface,
    swapchain: vk::SwapchainKHR,
}

impl Swapchain {
    pub fn new(
        device: &Arc<Device>,
        surface: Surface,
        info: SwapchainInfo,
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);

        Ok(Swapchain {
            device,
            images: vec![],
            info,
            next_semaphore: 0,
            acquired_semaphores: vec![],
            rendered_semaphores: vec![],
            suboptimal: true,
            surface,
            swapchain: vk::SwapchainKHR::null(),
        })
    }

    pub fn acquire_next_image(&mut self) -> Result<SwapchainImage, SwapchainError> {
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

    fn destroy(&mut self) {
        if self.swapchain != vk::SwapchainKHR::null() {
            unsafe {
                self.device
                    .swapchain_ext
                    .as_ref()
                    .unwrap()
                    .destroy_swapchain(self.swapchain, None);
            }

            self.swapchain = vk::SwapchainKHR::null();
        }
    }

    pub fn present_image(&mut self, image: SwapchainImage) {
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
        let desired_image_count =
            Self::clamp_desired_image_count(self.info.desired_image_count, surface_capabilities);

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
        let images: Vec<Option<Image>> = vk_images
            .into_iter()
            .enumerate()
            .map(|(idx, vk_image)| {
                let mut image = Image::from_raw(
                    &self.device,
                    vk_image,
                    ImageInfo {
                        ty: ImageType::Texture2D,
                        usage: surface_capabilities.supported_usage_flags,
                        flags: vk::ImageCreateFlags::empty(),
                        fmt: self.info.format.format,
                        depth: 1,
                        height: surface_height,
                        width: surface_width,
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

        self.info.height = surface_height;
        self.info.width = surface_width;
        self.next_semaphore = 0;
        self.images = images;
        self.swapchain = swapchain;

        while self.acquired_semaphores.len() < self.images.len() {
            self.acquired_semaphores.push(
                unsafe {
                    self.device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                }
                .unwrap(),
            );
            self.rendered_semaphores.push(
                unsafe {
                    self.device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                }
                .unwrap(),
            );
        }

        Ok(())
    }

    fn clamp_desired_image_count(
        desired_image_count: u32,
        surface_capabilities: vk::SurfaceCapabilitiesKHR,
    ) -> u32 {
        let mut desired_image_count = desired_image_count.max(surface_capabilities.min_image_count);

        if surface_capabilities.max_image_count != 0 {
            desired_image_count = desired_image_count.min(surface_capabilities.max_image_count);
        }

        desired_image_count
    }
}

impl Drop for Swapchain {
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
pub struct SwapchainImage {
    pub acquired: vk::Semaphore,
    pub image: Image,
    pub idx: u32,
    pub rendered: vk::Semaphore,
}

impl Clone for SwapchainImage {
    fn clone(&self) -> Self {
        Self {
            acquired: self.acquired,
            image: Image::clone_raw(&self.image),
            idx: self.idx,
            rendered: self.rendered,
        }
    }
}

impl Deref for SwapchainImage {
    type Target = Image;

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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SwapchainInfo {
    pub desired_image_count: u32,
    pub format: vk::SurfaceFormatKHR,
    pub height: u32,
    pub sync_display: bool,
    pub width: u32,
}
