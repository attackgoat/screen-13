use {
    super::{DriverError, Instance},
    ash::{extensions::khr, vk},
    log::warn,
    raw_window_handle::HasRawWindowHandle,
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        sync::Arc,
        thread::panicking,
    },
};

pub struct Surface {
    _instance: Arc<Instance>,
    surface: vk::SurfaceKHR,
    surface_ext: khr::Surface,
}

impl Surface {
    pub fn new(
        instance: &Arc<Instance>,
        window: &impl HasRawWindowHandle,
    ) -> Result<Self, DriverError> {
        let instance = Arc::clone(instance);
        let surface_ext = khr::Surface::new(&instance.entry, &instance);
        let surface =
            unsafe { ash_window::create_surface(&instance.entry, &instance, window, None) }
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;

        Ok(Self {
            _instance: instance,
            surface,
            surface_ext,
        })
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
            self.surface_ext.destroy_surface(self.surface, None);
        }
    }
}
