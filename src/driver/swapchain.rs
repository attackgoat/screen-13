//! Native window presentation types.

use {
    super::{
        DriverError, Surface,
        device::Device,
        image::{Image, ImageInfo},
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::{debug, info, trace, warn},
    std::{mem::replace, ops::Deref, slice, sync::Arc, thread::panicking},
};

// TODO: This needs to track completed command buffers and not constantly create semaphores

/// Provides the ability to present rendering results to a [`Surface`].
#[derive(Debug)]
pub struct Swapchain {
    device: Arc<Device>,
    images: Box<[SwapchainImage]>,
    info: SwapchainInfo,
    old_swapchain: vk::SwapchainKHR,
    suboptimal: bool,
    surface: Surface,
    swapchain: vk::SwapchainKHR,
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
            old_swapchain: vk::SwapchainKHR::null(),
            suboptimal: true,
            surface,
            swapchain: vk::SwapchainKHR::null(),
        })
    }

    /// Gets the next available swapchain image which should be rendered to and then presented using
    /// [`present_image`][Self::present_image].
    #[profiling::function]
    pub fn acquire_next_image(
        &mut self,
        acquired: vk::Semaphore,
    ) -> Result<SwapchainImage, SwapchainError> {
        for _ in 0..2 {
            if self.suboptimal {
                self.recreate_swapchain().map_err(|err| {
                    if matches!(err, DriverError::Unsupported) {
                        SwapchainError::Suboptimal
                    } else {
                        SwapchainError::SurfaceLost
                    }
                })?;
            }

            let swapchain_ext = Device::expect_swapchain_ext(&self.device);

            let image_idx = unsafe {
                swapchain_ext.acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    acquired,
                    vk::Fence::null(),
                )
            }
            .map(|(idx, suboptimal)| {
                if suboptimal {
                    debug!("acquired image is suboptimal");
                }

                self.suboptimal = suboptimal;

                idx
            });

            match image_idx {
                Ok(image_idx) => {
                    let image_idx = image_idx as usize;

                    assert!(image_idx < self.images.len());

                    let image = unsafe { self.images.get_unchecked(image_idx) };
                    let image = SwapchainImage::clone_swapchain(image);

                    return Ok(replace(
                        unsafe { self.images.get_unchecked_mut(image_idx) },
                        image,
                    ));
                }
                Err(err)
                    if err == vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT
                        || err == vk::Result::ERROR_OUT_OF_DATE_KHR
                        || err == vk::Result::NOT_READY
                        || err == vk::Result::TIMEOUT =>
                {
                    warn!("unable to acquire image: {err}");

                    self.suboptimal = true;

                    // Try again to see if we can acquire an image this frame
                    // (Makes redraw during resize look slightly better)
                    continue;
                }
                Err(err) if err == vk::Result::ERROR_DEVICE_LOST => {
                    warn!("unable to acquire image: {err}");

                    self.suboptimal = true;

                    return Err(SwapchainError::DeviceLost);
                }
                Err(err) if err == vk::Result::ERROR_SURFACE_LOST_KHR => {
                    warn!("unable to acquire image: {err}");

                    self.suboptimal = true;

                    return Err(SwapchainError::SurfaceLost);
                }
                Err(err) => {
                    // Probably:
                    // VK_ERROR_OUT_OF_HOST_MEMORY
                    // VK_ERROR_OUT_OF_DEVICE_MEMORY

                    // TODO: Maybe handle timeout in here

                    warn!("unable to acquire image: {err}");

                    return Err(SwapchainError::SurfaceLost);
                }
            }
        }

        Err(SwapchainError::Suboptimal)
    }

    fn clamp_desired_image_count(
        desired_image_count: u32,
        surface_capabilities: vk::SurfaceCapabilitiesKHR,
    ) -> u32 {
        let mut desired_image_count = desired_image_count.max(surface_capabilities.min_image_count);

        if surface_capabilities.max_image_count != 0 {
            desired_image_count = desired_image_count.min(surface_capabilities.max_image_count);
        }

        desired_image_count.min(u8::MAX as u32)
    }

    #[profiling::function]
    fn destroy_swapchain(device: &Device, swapchain: &mut vk::SwapchainKHR) {
        // TODO: Any cases where we need to wait for idle here?

        if *swapchain != vk::SwapchainKHR::null() {
            let swapchain_ext = Device::expect_swapchain_ext(device);

            unsafe {
                swapchain_ext.destroy_swapchain(*swapchain, None);
            }

            *swapchain = vk::SwapchainKHR::null();
        }
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
        wait_semaphores: &[vk::Semaphore],
        queue_family_index: u32,
        queue_index: u32,
    ) {
        let queue_family_index = queue_family_index as usize;
        let queue_index = queue_index as usize;

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

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(wait_semaphores)
            .swapchains(slice::from_ref(&self.swapchain))
            .image_indices(slice::from_ref(&image.image_idx));

        let swapchain_ext = Device::expect_swapchain_ext(&self.device);

        unsafe {
            match swapchain_ext.queue_present(
                self.device.queues[queue_family_index][queue_index],
                &present_info,
            ) {
                Ok(_) => {
                    Self::destroy_swapchain(&self.device, &mut self.old_swapchain);
                }
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
        self.images[image_idx] = image;
    }

    #[profiling::function]
    fn recreate_swapchain(&mut self) -> Result<(), DriverError> {
        Self::destroy_swapchain(&self.device, &mut self.old_swapchain);

        let (surface_capabilities, present_modes) = {
            let surface_ext = Device::expect_surface_ext(&self.device);
            let surface_capabilities = unsafe {
                surface_ext.get_physical_device_surface_capabilities(
                    *self.device.physical_device,
                    *self.surface,
                )
            }
            .inspect_err(|err| warn!("unable to get surface capabilities: {err}"))
            .or(Err(DriverError::Unsupported))?;

            let present_modes = unsafe {
                surface_ext.get_physical_device_surface_present_modes(
                    *self.device.physical_device,
                    *self.surface,
                )
            }
            .inspect_err(|err| warn!("unable to get surface present modes: {err}"))
            .or(Err(DriverError::Unsupported))?;

            (surface_capabilities, present_modes)
        };

        let desired_image_count =
            Self::clamp_desired_image_count(self.info.desired_image_count, surface_capabilities);

        let image_usage =
            self.supported_surface_usage(surface_capabilities.supported_usage_flags)?;

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

        let swapchain_ext = Device::expect_swapchain_ext(&self.device);
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(*self.surface)
            .min_image_count(desired_image_count)
            .image_color_space(self.info.surface.color_space)
            .image_format(self.info.surface.format)
            .image_extent(vk::Extent2D {
                width: surface_width,
                height: surface_height,
            })
            .image_usage(image_usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(self.swapchain)
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
                        image_usage,
                    ),
                );

                let image_idx = image_idx as u32;
                image.name = Some(format!("swapchain{image_idx}"));

                Ok(SwapchainImage {
                    exec_idx: 0,
                    image,
                    image_idx,
                })
            })
            .collect::<Result<Box<_>, _>>()?;

        debug_assert_eq!(desired_image_count, images.len() as u32);

        self.info.height = surface_height;
        self.info.width = surface_width;
        self.images = images;
        self.old_swapchain = self.swapchain;
        self.swapchain = swapchain;
        self.suboptimal = false;

        info!(
            "swapchain {}x{} {present_mode:?}x{} {:?} {image_usage:#?}",
            self.info.width,
            self.info.height,
            self.images.len(),
            self.info.surface.format,
        );

        Ok(())
    }

    /// Sets information about this swapchain.
    ///
    /// Previously acquired swapchain images should be discarded after calling this function.
    pub fn set_info(&mut self, info: impl Into<SwapchainInfo>) {
        let info: SwapchainInfo = info.into();

        if self.info != info {
            self.info = info;

            trace!("info: {:?}", self.info);

            self.suboptimal = true;
        }
    }

    fn supported_surface_usage(
        &mut self,
        surface_capabilities: vk::ImageUsageFlags,
    ) -> Result<vk::ImageUsageFlags, DriverError> {
        let mut res = vk::ImageUsageFlags::empty();

        for bit in 0..u32::BITS {
            let usage = vk::ImageUsageFlags::from_raw((1 << bit) & surface_capabilities.as_raw());
            if usage.is_empty() {
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
            .inspect_err(|err| {
                warn!(
                    "unable to get image format properties: {:?} {:?} {err}",
                    self.info.surface.format, usage
                )
            })?
            .is_none()
            {
                continue;
            }

            res |= usage;
        }

        // On mesa the device will return this usage flag as supported even when the extension
        // that is needed for an image to have this flag isn't enabled
        res &= !vk::ImageUsageFlags::ATTACHMENT_FEEDBACK_LOOP_EXT;

        Ok(res)
    }
}

impl Drop for Swapchain {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        Self::destroy_swapchain(&self.device, &mut self.old_swapchain);
        Self::destroy_swapchain(&self.device, &mut self.swapchain);
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

/// An opaque type representing a swapchain image.
#[derive(Debug)]
pub struct SwapchainImage {
    pub(crate) exec_idx: usize,
    image: Image,
    image_idx: u32,
}

impl SwapchainImage {
    pub(crate) fn clone_swapchain(this: &Self) -> Self {
        let Self {
            exec_idx,
            image,
            image_idx,
        } = this;

        Self {
            exec_idx: *exec_idx,
            image: Image::clone_swapchain(image),
            image_idx: *image_idx,
        }
    }
}

impl Deref for SwapchainImage {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
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
    pub fn swapchain_info_builder_uninit_height() {
        Builder::default().build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: surface")]
    pub fn swapchain_info_builder_uninit_surface() {
        Builder::default().height(42).build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: width")]
    pub fn swapchain_info_builder_uninit_width() {
        Builder::default()
            .height(42)
            .surface(vk::SurfaceFormatKHR::default())
            .build();
    }
}
