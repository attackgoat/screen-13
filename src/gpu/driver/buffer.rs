use {
    super::Driver,
    gfx_hal::{buffer::Usage, device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Buffer {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::Buffer>,
}

impl Buffer {
    pub fn new(
        #[cfg(feature = "debug-names")] name: &str,
        driver: Driver,
        usage: Usage,
        len: u64,
    ) -> Self {
        let buffer = {
            let device = driver.borrow();
            let ctor = || unsafe { device.create_buffer(len as u64, usage).unwrap() };

            #[cfg(feature = "debug-names")]
            let mut buffer = ctor();

            #[cfg(not(feature = "debug-names"))]
            let buffer = ctor();

            #[cfg(feature = "debug-names")]
            unsafe {
                device.set_buffer_name(&mut buffer, name);
            }

            buffer
        };

        Self {
            ptr: Some(buffer),
            driver,
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(buf: &mut Self, name: &str) {
        let device = buf.driver.borrow();
        let ptr = buf.ptr.as_mut().unwrap();

        unsafe {
            device.set_buffer_name(ptr, name);
        }
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
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_buffer(ptr);
        }
    }
}
