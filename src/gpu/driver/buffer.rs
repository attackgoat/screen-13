use {
    crate::gpu::device,
    gfx_hal::{buffer::Usage, device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Buffer(Option<<_Backend as Backend>::Buffer>);

impl Buffer {
    pub unsafe fn new(#[cfg(feature = "debug-names")] name: &str, usage: Usage, len: u64) -> Self {
        let ctor = || device().create_buffer(len as u64, usage).unwrap();

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_buffer_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(buf: &mut Self, name: &str) {
        let ptr = buf.0.as_mut().unwrap();
        device().set_buffer_name(ptr, name);
    }
}

impl AsMut<<_Backend as Backend>::Buffer> for Buffer {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Buffer {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Buffer> for Buffer {
    fn as_ref(&self) -> &<_Backend as Backend>::Buffer {
        &*self
    }
}

impl Deref for Buffer {
    type Target = <_Backend as Backend>::Buffer;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_buffer(ptr);
        }
    }
}
