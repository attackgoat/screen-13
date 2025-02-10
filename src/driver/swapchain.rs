//! Native window presentation types.

use {
    super::{
        device::Device,
        image::{Image, ImageInfo},
        AccessType, DriverError, Surface,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::{debug, info, warn},
    std::{ops::Deref, slice, sync::Arc, thread::panicking},
};

/// Provides the ability to present rendering results to a [`Surface`].
#[derive(Debug)]
pub struct Swapchain {
    device: Arc<Device>,
    images: Vec<Option<SwapchainImage>>,
    info: SwapchainInfo,
    suboptimal: bool,
    surface: Surface,
    swapchain: vk::SwapchainKHR,
    sync_idx: usize,
    syncs: Vec<Synchronization>,
}

impl Swapchain {
    /// Prepares a [`vk::SwapchainKHR`] object which is lazily created after calling
    /// [`acquire_next_image`][Self::acquire_next_image].
    #[profiling::function]
    pub fn new(
        device: &Arc<Device>,
        surface: Surface,
        info: impl Into<SwapchainInfo>,
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let info = info.into();

        Ok(Swapchain {
            device,
            images: Default::default(),
            info,
            suboptimal: true,
            surface,
            swapchain: vk::SwapchainKHR::null(),
            sync_idx: 0,
            syncs: Default::default(),
        })
    }

    /// Gets the next available swapchain image which should be rendered to and then presented using
    /// [`present_image`][Self::present_image].
    #[profiling::function]
    pub fn acquire_next_image(&mut self) -> Result<SwapchainImage, SwapchainError> {
        if self.suboptimal {
            self.recreate_swapchain()
                .map_err(|_| SwapchainError::SurfaceLost)?;
            self.suboptimal = false;
        }

        let mut acquired = vk::Semaphore::null();
        let mut ready = vk::Fence::null();

        for idx in 0..self.syncs.len() {
            self.sync_idx += 1;
            self.sync_idx %= self.syncs.len();
            let sync = &self.syncs[idx];

            unsafe {
                match self.device.get_fence_status(sync.ready) {
                    Ok(true) => {
                        if self
                            .device
                            .reset_fences(slice::from_ref(&sync.ready))
                            .is_err()
                        {
                            self.suboptimal = true;
                            return Err(SwapchainError::DeviceLost);
                        }

                        acquired = sync.acquired;
                        ready = sync.ready;

                        // info!("Sync idx {}", self.sync_idx);
                    }
                    Ok(false) => continue,
                    Err(_) => {
                        self.suboptimal = true;
                        return Err(SwapchainError::DeviceLost);
                    }
                }

                if self
                    .device
                    .reset_fences(slice::from_ref(&sync.ready))
                    .is_err()
                {
                    self.suboptimal = true;
                    return Err(SwapchainError::DeviceLost);
                }
            }
        }

        if acquired == vk::Semaphore::null() {
            let sync = Synchronization::create(&self.device).map_err(|err| {
                warn!("{err}");

                SwapchainError::DeviceLost
            })?;
            acquired = sync.acquired;
            ready = sync.ready;

            self.sync_idx = self.syncs.len();
            self.syncs.push(sync);
        }

        let image_idx = unsafe {
            // We checked during recreate_swapchain
            let swapchain_ext = self.device.swapchain_ext.as_ref().unwrap_unchecked();

            swapchain_ext.acquire_next_image(self.swapchain, u64::MAX, acquired, ready)
        }
        .map(|(idx, suboptimal)| {
            if suboptimal {
                self.suboptimal = true;
            }

            idx
        });

        match image_idx {
            Ok(image_idx) => {
                let mut image = self.images[image_idx as usize].take().ok_or_else(|| {
                    self.suboptimal = true;

                    SwapchainError::Suboptimal
                })?;

                image.acquired = acquired;

                Ok(image)
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

    #[profiling::function]
    fn destroy(&mut self) {
        if self.swapchain != vk::SwapchainKHR::null() {
            unsafe {
                // We checked when creating the swapchain
                let swapchain_ext = self.device.swapchain_ext.as_ref().unwrap_unchecked();

                swapchain_ext.destroy_swapchain(self.swapchain, None);
            }

            self.swapchain = vk::SwapchainKHR::null();
        }

        for Synchronization { acquired, ready } in self.syncs.drain(..) {
            // if let Err(err) = Device::wait_for_fence(&self.device, &ready) {
            //     warn!("{err}");
            // }

            unsafe {
                self.device.destroy_semaphore(acquired, None);
                self.device.destroy_fence(ready, None);
            }
        }

        self.images.clear();
    }

    /// Gets information about this swapchain.
    pub fn info(&self) -> SwapchainInfo {
        self.info
    }

    /// Presents an image which has been previously acquired using
    /// [`acquire_next_image`][Self::acquire_next_image].
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

        // We checked when handling out the swapchain image
        let swapchain_ext = unsafe { self.device.swapchain_ext.as_ref().unwrap_unchecked() };

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(slice::from_ref(&image.rendered))
            .swapchains(slice::from_ref(&self.swapchain))
            .image_indices(slice::from_ref(&image.image_idx));

        unsafe {
            match swapchain_ext.queue_present(
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
                Err(err) => {
                    // Probably:
                    // VK_ERROR_OUT_OF_HOST_MEMORY
                    // VK_ERROR_OUT_OF_DEVICE_MEMORY
                    warn!("{err}");
                }
            }
        }

        let image_idx = image.image_idx as usize;

        debug_assert!(self.images[image_idx].is_none());

        self.images[image_idx] = Some(image);
    }

    #[profiling::function]
    fn recreate_swapchain(&mut self) -> Result<(), DriverError> {
        if let Err(err) = unsafe { self.device.device_wait_idle() } {
            warn!("device_wait_idle() failed: {err}");
        }

        self.destroy();

        let surface_ext = self.device.surface_ext.as_ref().ok_or_else(|| {
            warn!("Unsupported surface extension");

            DriverError::Unsupported
        })?;

        let mut surface_capabilities = unsafe {
            surface_ext.get_physical_device_surface_capabilities(
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

            if Device::image_format_properties(
                &self.device,
                self.info.surface.format,
                vk::ImageType::TYPE_2D,
                vk::ImageTiling::OPTIMAL,
                usage,
                vk::ImageCreateFlags::empty(),
            )
            .is_err()
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

        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        info!(
            "supported_usage_flags {:#?}",
            &surface_capabilities.supported_usage_flags
        );

        let swapchain_ext = self.device.swapchain_ext.as_ref().ok_or_else(|| {
            warn!("Unsupported swapchain extension");

            DriverError::Unsupported
        })?;

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(*self.surface)
            .min_image_count(desired_image_count)
            .image_color_space(self.info.surface.color_space)
            .image_format(self.info.surface.format)
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
            .image_array_layers(1);
        let swapchain = unsafe { swapchain_ext.create_swapchain(&swapchain_create_info, None) }
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;

        let images =
            unsafe { swapchain_ext.get_swapchain_images(swapchain) }.map_err(|err| match err {
                vk::Result::INCOMPLETE => DriverError::InvalidData,
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                    DriverError::OutOfMemory
                }
                _ => DriverError::Unsupported,
            })?;
        let images = images
            .into_iter()
            .enumerate()
            .map(|(image_idx, image)| {
                let mut image = Image::from_raw(
                    &self.device,
                    image,
                    ImageInfo::image_2d(
                        surface_width,
                        surface_height,
                        self.info.surface.format,
                        surface_capabilities.supported_usage_flags,
                    ),
                );

                let image_idx = image_idx as u32;
                image.name = Some(format!("swapchain{image_idx}"));

                let rendered = Device::create_semaphore(&self.device)?;

                Ok(Some(SwapchainImage {
                    image,
                    image_idx,
                    acquired: vk::Semaphore::null(),
                    rendered,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

        debug_assert_eq!(desired_image_count, images.len() as u32);

        self.info.height = surface_height;
        self.info.width = surface_width;
        self.images = images;
        self.swapchain = swapchain;
        self.sync_idx = 0;

        info!(
            "Swapchain {}x{} {:?} {present_mode:?}x{}",
            self.info.width,
            self.info.height,
            self.info.surface.format,
            self.images.len(),
        );

        Ok(())
    }

    /// Sets information about this swapchain.
    ///
    /// Previously acquired swapchain images should be discarded after calling this function.
    pub fn set_info(&mut self, info: SwapchainInfo) {
        if self.info != info {
            self.info = info;
            self.suboptimal = true;
        }
    }
}

impl Drop for Swapchain {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        self.destroy();
    }
}

/// An opaque type representing a swapchain image.
#[derive(Debug)]
pub struct SwapchainImage {
    pub(crate) acquired: vk::Semaphore,
    image: Image,
    image_idx: u32,
    pub(crate) rendered: vk::Semaphore,
}

impl SwapchainImage {
    pub(crate) fn access(
        &mut self,
        access: AccessType,
        range: vk::ImageSubresourceRange,
    ) -> impl Iterator<Item = (AccessType, vk::ImageSubresourceRange)> + '_ {
        Image::access(self, access, range)
    }

    pub(crate) fn unbind(&mut self) -> Self {
        let &mut Self {
            acquired,
            image_idx,
            rendered,
            ..
        } = self;

        self.rendered = vk::Semaphore::null();

        Self {
            acquired,
            image: Image::clone_raw(&self.image),
            image_idx,
            rendered,
        }
    }
}

impl Drop for SwapchainImage {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        if self.rendered != vk::Semaphore::null() {
            unsafe {
                self.image.device.destroy_semaphore(self.rendered, None);
            }
        }
    }
}

impl Deref for SwapchainImage {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

/// Describes the condition of a swapchain.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    derive(Clone, Copy, Debug),
    pattern = "owned"
)]
#[non_exhaustive]
pub struct SwapchainInfo {
    /// The desired, but not guaranteed, number of images that will be in the created swapchain.
    ///
    /// More images introduces more display lag, but smoother animation.
    #[builder(default = "3")]
    pub desired_image_count: u32,

    /// The initial height of the surface.
    pub height: u32,

    /// The format and color space of the surface.
    pub surface: vk::SurfaceFormatKHR,

    /// Determines if frames will be submitted to the display in a synchronous fashion or if they
    /// should be displayed as fast as possible instead.
    ///
    /// Turn on to eliminate visual tearing at the expense of latency.
    #[builder(default = "true")]
    pub sync_display: bool,

    /// The initial width of the surface.
    pub width: u32,
}

impl SwapchainInfo {
    /// Specifies a default swapchain with the given `width`, `height` and `format` values.
    #[inline(always)]
    pub const fn new(width: u32, height: u32, surface: vk::SurfaceFormatKHR) -> SwapchainInfo {
        Self {
            width,
            height,
            surface,
            desired_image_count: 3,
            sync_display: true,
        }
    }

    /// Converts a `SwapchainInfo` into a `SwapchainInfoBuilder`.
    #[inline(always)]
    pub fn to_builder(self) -> SwapchainInfoBuilder {
        SwapchainInfoBuilder {
            desired_image_count: Some(self.desired_image_count),
            height: Some(self.height),
            surface: Some(self.surface),
            sync_display: Some(self.sync_display),
            width: Some(self.width),
        }
    }
}

impl From<SwapchainInfoBuilder> for SwapchainInfo {
    fn from(info: SwapchainInfoBuilder) -> Self {
        info.build()
    }
}

impl SwapchainInfoBuilder {
    /// Builds a new `SwapchainInfo`.
    ///
    /// # Panics
    ///
    /// If any of the following values have not been set this function will panic:
    ///
    /// * `width`
    /// * `height`
    /// * `surface`
    #[inline(always)]
    pub fn build(self) -> SwapchainInfo {
        match self.fallible_build() {
            Err(SwapchainInfoBuilderError(err)) => panic!("{err}"),
            Ok(info) => info,
        }
    }
}

#[derive(Debug)]
struct SwapchainInfoBuilderError(UninitializedFieldError);

impl From<UninitializedFieldError> for SwapchainInfoBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(err)
    }
}

#[derive(Debug)]
struct Synchronization {
    acquired: vk::Semaphore,
    ready: vk::Fence,
}

impl Synchronization {
    fn create(device: &Device) -> Result<Self, DriverError> {
        let acquired = Device::create_semaphore(device)?;
        let ready = Device::create_fence(device, false)?;

        Ok(Self { acquired, ready })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Info = SwapchainInfo;
    type Builder = SwapchainInfoBuilder;

    #[test]
    pub fn swapchain_info() {
        let info = Info::new(20, 24, vk::SurfaceFormatKHR::default());
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn swapchain_info_builder() {
        let info = Info::new(23, 64, vk::SurfaceFormatKHR::default());
        let builder = Builder::default()
            .width(23)
            .height(64)
            .surface(vk::SurfaceFormatKHR::default())
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: height")]
    pub fn accel_struct_info_builder_uninit_height() {
        Builder::default().build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: surface")]
    pub fn accel_struct_info_builder_uninit_surface() {
        Builder::default().height(42).build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: width")]
    pub fn accel_struct_info_builder_uninit_width() {
        Builder::default()
            .height(42)
            .surface(vk::SurfaceFormatKHR::default())
            .build();
    }
}
