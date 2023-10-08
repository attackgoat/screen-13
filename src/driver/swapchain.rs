use {
    super::{
        device::Device,
        image::{Image, ImageInfo, ImageType, SampleCount},
        DriverError, Surface,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::{debug, info, warn},
    std::{ops::Deref, slice, sync::Arc, thread::panicking, time::Duration},
};

#[derive(Debug)]
pub struct Swapchain {
    device: Arc<Device>,
    images: Vec<Option<Image>>,
    info: SwapchainInfo,
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
        info: impl Into<SwapchainInfo>,
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let info = info.into();

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

    #[profiling::function]
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

    pub fn info(&self) -> SwapchainInfo {
        self.info
    }

    #[profiling::function]
    pub fn present_image(
        &mut self,
        image: SwapchainImage,
        queue_family_index: usize,
        queue_index: usize,
    ) {
        debug_assert!(
            queue_family_index < self.device.physical_device.queue_families.len(),
            "Queue family index must be within the range of the available queues created by the device."
        );
        debug_assert!(
            queue_index
                < self.device.physical_device.queue_families[queue_family_index].queue_count
                    as usize,
            "Queue index must be within the range of the available queues created by the device."
        );

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(slice::from_ref(&image.rendered))
            .swapchains(slice::from_ref(&self.swapchain))
            .image_indices(slice::from_ref(&image.idx));

        unsafe {
            match self.device.swapchain_ext.as_ref().unwrap().queue_present(
                self.device.queues[queue_family_index][queue_index],
                &present_info,
            ) {
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

        let mut surface_capabilities = unsafe {
            self.device
                .surface_ext
                .as_ref()
                .unwrap()
                .get_physical_device_surface_capabilities(
                    *self.device.physical_device,
                    *self.surface,
                )
        }
        .map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;

        // TODO: When ash flags support iter() we can simplify this!
        for usage in [
            vk::ImageUsageFlags::ATTACHMENT_FEEDBACK_LOOP_EXT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageUsageFlags::FRAGMENT_DENSITY_MAP_EXT,
            vk::ImageUsageFlags::FRAGMENT_SHADING_RATE_ATTACHMENT_KHR,
            vk::ImageUsageFlags::INPUT_ATTACHMENT,
            vk::ImageUsageFlags::INVOCATION_MASK_HUAWEI,
            vk::ImageUsageFlags::RESERVED_16_QCOM,
            vk::ImageUsageFlags::RESERVED_17_QCOM,
            vk::ImageUsageFlags::RESERVED_22_EXT,
            // vk::ImageUsageFlags::RESERVED_23_EXT,
            vk::ImageUsageFlags::SAMPLED,
            vk::ImageUsageFlags::SAMPLE_BLOCK_MATCH_QCOM,
            vk::ImageUsageFlags::SAMPLE_WEIGHT_QCOM,
            vk::ImageUsageFlags::SHADING_RATE_IMAGE_NV,
            vk::ImageUsageFlags::STORAGE,
            vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageUsageFlags::TRANSFER_SRC,
            vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR,
            vk::ImageUsageFlags::VIDEO_DECODE_DST_KHR,
            vk::ImageUsageFlags::VIDEO_DECODE_SRC_KHR,
            vk::ImageUsageFlags::VIDEO_ENCODE_DPB_KHR,
            vk::ImageUsageFlags::VIDEO_ENCODE_DST_KHR,
            vk::ImageUsageFlags::VIDEO_ENCODE_SRC_KHR,
        ] {
            if !surface_capabilities.supported_usage_flags.contains(usage) {
                continue;
            }

            if !Device::image_format_properties(
                &self.device,
                self.info.format.format,
                vk::ImageType::TYPE_2D,
                vk::ImageTiling::OPTIMAL,
                usage,
                vk::ImageCreateFlags::empty(),
            )
            .is_ok()
            {
                surface_capabilities.supported_usage_flags &= !usage;
            }
        }

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
            self.device
                .surface_ext
                .as_ref()
                .unwrap()
                .get_physical_device_surface_present_modes(
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
        let swapchain = unsafe {
            self.device
                .swapchain_ext
                .as_ref()
                .unwrap()
                .create_swapchain(&swapchain_create_info, None)
        }
        .map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;

        let vk_images = unsafe {
            self.device
                .swapchain_ext
                .as_ref()
                .unwrap()
                .get_swapchain_images(swapchain)
        }
        .unwrap();
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

        info!(
            "Swapchain {}x{} {:?} {present_mode:?}",
            self.info.width, self.info.height, self.info.format.format
        );

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

    pub fn set_info(&mut self, info: SwapchainInfo) {
        if self.info != info {
            self.info = info;
            self.suboptimal = true;
        }
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

        unsafe {
            self.device.device_wait_idle().unwrap_or_default();
        };

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

/// Information used to create a [`Swapchain`] instance.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build", error = "SwapchainInfoBuilderError"),
    derive(Debug),
    pattern = "owned"
)]
pub struct SwapchainInfo {
    /// The desired, but not guaranteed, number of images that will be in the created swapchain.
    ///
    /// More images introduces more display lag, but smoother animation.
    #[builder(default = "3")]
    pub desired_image_count: u32,

    /// The format of the surface.
    pub format: vk::SurfaceFormatKHR,

    /// The initial height of the surface.
    #[builder(default = "8")]
    pub height: u32,

    /// Determines if frames will be submitted to the display in a synchronous fashion or if they
    /// should be displayed as fast as possible instead.
    ///
    /// Turn on to eliminate visual tearing at the expense of latency.
    #[builder(default = "true")]
    pub sync_display: bool,

    /// The initial width of the surface.
    #[builder(default = "8")]
    pub width: u32,
}

impl SwapchainInfo {
    /// Specifies default device information.
    #[allow(clippy::new_ret_no_self, unused)]
    pub fn new(width: u32, height: u32, format: vk::SurfaceFormatKHR) -> SwapchainInfoBuilder {
        SwapchainInfoBuilder::default()
            .width(width)
            .height(height)
            .format(format)
    }
}

impl From<SwapchainInfoBuilder> for SwapchainInfo {
    fn from(info: SwapchainInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl SwapchainInfoBuilder {
    /// Builds a new `SwapchainInfo`.
    pub fn build(self) -> SwapchainInfo {
        self.fallible_build().unwrap()
    }
}

#[derive(Debug)]
struct SwapchainInfoBuilderError;

impl From<UninitializedFieldError> for SwapchainInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}
