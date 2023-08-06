use {
    super::{device::Device, DriverError, Instance},
    ash::vk,
    ash_window::create_surface,
    log::error,
    raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle},
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        sync::Arc,
        thread::panicking,
    },
};

pub struct Surface {
    device: Arc<Device>,
    surface: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(
        device: &Arc<Device>,
        display_window: &(impl HasRawDisplayHandle + HasRawWindowHandle),
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let instance = Device::instance(&device);
        let surface = unsafe {
            create_surface(
                Instance::entry(instance),
                instance,
                display_window.raw_display_handle(),
                display_window.raw_window_handle(),
                None,
            )
        }
        .map_err(|err| {
            error!("unable to create surface: {err}");

            DriverError::Unsupported
        })?;

        Ok(Self { device, surface })
    }

    pub fn formats(this: &Self) -> Result<Vec<vk::SurfaceFormatKHR>, DriverError> {
        unsafe {
            this.device
                .surface_ext
                .as_ref()
                .unwrap()
                .get_physical_device_surface_formats(*this.device.physical_device, this.surface)
                .map_err(|err| {
                    error!("unable to get surface formats: {err}");

                    DriverError::Unsupported
                })
        }
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
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device
                .surface_ext
                .as_ref()
                .unwrap()
                .destroy_surface(self.surface, None);
        }
    }
}
