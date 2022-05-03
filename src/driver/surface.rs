use {
    super::{DriverError, Instance},
    archery::{SharedPointer, SharedPointerKind},
    ash::{extensions::khr, vk},
    log::warn,
    raw_window_handle::HasRawWindowHandle,
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        thread::panicking,
    },
};

pub struct Surface<P>
where
    P: SharedPointerKind,
{
    _instance: SharedPointer<Instance, P>,
    surface: vk::SurfaceKHR,
    surface_ext: khr::Surface,
}

impl<P> Surface<P>
where
    P: SharedPointerKind,
{
    pub fn new(
        instance: &SharedPointer<Instance, P>,
        window: &impl HasRawWindowHandle,
    ) -> Result<Self, DriverError> {
        let instance = SharedPointer::clone(instance);
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

impl<P> Debug for Surface<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Surface")
    }
}

impl<P> Deref for Surface<P>
where
    P: SharedPointerKind,
{
    type Target = vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl<P> Drop for Surface<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.surface_ext.destroy_surface(self.surface, None);
        }
    }
}
