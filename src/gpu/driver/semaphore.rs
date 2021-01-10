use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Semaphore(Option<<_Backend as Backend>::Semaphore>);

impl Semaphore {
    pub unsafe fn new(#[cfg(feature = "debug-names")] name: &str) -> Self {
        let ctor = || device().create_semaphore().unwrap();

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_semaphore_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(semaphore: &mut Self, name: &str) {
        let ptr = semaphore.0.as_mut().unwrap();
        device().set_semaphore_name(ptr, name);
    }
}

impl AsMut<<_Backend as Backend>::Semaphore> for Semaphore {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Semaphore {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Semaphore> for Semaphore {
    fn as_ref(&self) -> &<_Backend as Backend>::Semaphore {
        &*self
    }
}

impl Deref for Semaphore {
    type Target = <_Backend as Backend>::Semaphore;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for Semaphore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_semaphore(ptr);
        }
    }
}
