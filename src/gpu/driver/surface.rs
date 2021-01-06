use {
    crate::gpu::instance,
    gfx_hal::{window::InitError, Backend, Instance as _},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
    winit::window::Window,
};

pub struct Surface(Option<<_Backend as Backend>::Surface>);

impl Surface {
    pub unsafe fn new(window: &Window) -> Result<Self, InitError> {
        let ptr = instance().create_surface(window)?;

        Ok(Self(Some(ptr)))
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for Surface {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            instance().destroy_surface(ptr);
        }
    }
}
