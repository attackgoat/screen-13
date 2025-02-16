//! Native platform window surface types.

use {
    super::{device::Device, DriverError, Instance},
    ash::vk,
    ash_window::create_surface,
    log::warn,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        sync::Arc,
        thread::panicking,
    },
};

/// Smart pointer handle to a [`vk::SurfaceKHR`] object.
pub struct Surface {
    device: Arc<Device>,
    surface: vk::SurfaceKHR,
}

impl Surface {
    /// Create a surface from a raw window display handle.
    ///
    /// `device` must have been created with platform specific surface extensions enabled, acquired
    /// through [`Device::create_display_window`].
    #[profiling::function]
    pub fn create(
        device: &Arc<Device>,
        window: &(impl HasDisplayHandle + HasWindowHandle),
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let instance = Device::instance(&device);
        let display_handle = window.display_handle().map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;
        let window_handle = window.window_handle().map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;
        let surface = unsafe {
            create_surface(
                Instance::entry(instance),
                instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
        }
        .map_err(|err| {
            warn!("Unable to create surface: {err}");

            DriverError::Unsupported
        })?;

        Ok(Self { device, surface })
    }

    /// Lists the supported surface formats.
    #[profiling::function]
    pub fn formats(this: &Self) -> Result<Vec<vk::SurfaceFormatKHR>, DriverError> {
        unsafe {
            this.device
                .surface_ext
                .as_ref()
                .unwrap()
                .get_physical_device_surface_formats(*this.device.physical_device, this.surface)
                .map_err(|err| {
                    warn!("Unable to get surface formats: {err}");

                    DriverError::Unsupported
                })
        }
    }

    /// Helper function to automatically select the best UNORM format, if one is available.
    #[profiling::function]
    pub fn linear(formats: &[vk::SurfaceFormatKHR]) -> Option<vk::SurfaceFormatKHR> {
        formats
            .iter()
            .find(|&&vk::SurfaceFormatKHR { format, .. }| {
                matches!(
                    format,
                    vk::Format::R8G8B8A8_UNORM | vk::Format::B8G8R8A8_UNORM
                )
            })
            .copied()
    }

    /// Helper function to automatically select the best UNORM format.
    ///
    /// **_NOTE:_** The default surface format is undefined, and although legal the results _may_
    /// not support presentation. You should prefer to use [`Surface::linear`] and fall back to
    /// supported values manually.
    pub fn linear_or_default(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        Self::linear(formats).unwrap_or_else(|| formats.first().copied().unwrap_or_default())
    }

    /// Helper function to automatically select the best sRGB format, if one is available.
    #[profiling::function]
    pub fn srgb(formats: &[vk::SurfaceFormatKHR]) -> Option<vk::SurfaceFormatKHR> {
        formats
            .iter()
            .find(
                |&&vk::SurfaceFormatKHR {
                     color_space,
                     format,
                 }| {
                    matches!(color_space, vk::ColorSpaceKHR::SRGB_NONLINEAR)
                        && matches!(
                            format,
                            vk::Format::R8G8B8A8_SRGB | vk::Format::B8G8R8A8_SRGB
                        )
                },
            )
            .copied()
    }

    /// Helper function to automatically select the best sRGB format.
    ///
    /// **_NOTE:_** The default surface format is undefined, and although legal the results _may_
    /// not support presentation. You should prefer to use [`Surface::srgb`] and fall back to
    /// supported values manually.
    pub fn srgb_or_default(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        Self::srgb(formats).unwrap_or_else(|| formats.first().copied().unwrap_or_default())
    }
}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Surface")
    }
}

impl Deref for Surface {
    type Target = vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl Drop for Surface {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        let surface_ext = Device::expect_surface_ext(&self.device);

        unsafe {
            surface_ext.destroy_surface(self.surface, None);
        }
    }
}
