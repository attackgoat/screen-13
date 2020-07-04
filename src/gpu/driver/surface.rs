use {
    crate::Error,
    gfx_hal::{Backend, Instance},
    gfx_impl::{Backend as _Backend, Instance as _InstanceImpl},
    std::ops::{Deref, DerefMut},
    winit::window::Window,
};

#[derive(Debug)]
pub struct Surface {
    instance: Option<_InstanceImpl>,
    surface: Option<<_Backend as Backend>::Surface>,
}

impl Surface {
    pub fn new(instance: _InstanceImpl, window: &Window) -> Result<Self, Error> {
        let surface = unsafe { instance.create_surface(window)? };
        Surface::existing(Some(instance), surface)
    }

    pub fn existing(
        instance: Option<_InstanceImpl>,
        surface: <_Backend as Backend>::Surface,
    ) -> Result<Self, Error> {
        Ok(Self {
            instance,
            surface: Some(surface),
        })
    }
}

impl Deref for Surface {
    type Target = <_Backend as Backend>::Surface;

    fn deref(&self) -> &Self::Target {
        self.surface.as_ref().unwrap()
    }
}

impl DerefMut for Surface {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.surface.as_mut().unwrap()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(instance) = &self.instance {
            unsafe {
                instance.destroy_surface(self.surface.take().unwrap());
            }
        }
    }
}
