use {
    gfx_hal::{window::InitError, Backend, Instance as _},
    gfx_impl::{Backend as _Backend, Instance},
    std::ops::{Deref, DerefMut},
    winit::window::Window,
};

pub struct Surface {
    instance: Option<Instance>,
    ptr: Option<<_Backend as Backend>::Surface>,
}

impl Surface {
    pub fn new(instance: Instance, window: &Window) -> Result<Self, InitError> {
        let surface = unsafe { instance.create_surface(window)? };

        Ok(Self {
            instance: Some(instance),
            ptr: Some(surface),
        })
    }
}

impl AsMut<<_Backend as Backend>::Surface> for Surface {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Surface {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Surface> for Surface {
    fn as_ref(&self) -> &<_Backend as Backend>::Surface {
        &*self
    }
}

impl Deref for Surface {
    type Target = <_Backend as Backend>::Surface;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Surface {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(instance) = &self.instance {
            let ptr = self.ptr.take().unwrap();

            unsafe {
                instance.destroy_surface(ptr);
            }
        }
    }
}
