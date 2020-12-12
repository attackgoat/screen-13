use {
    super::{Device, Driver, Memory},
    gfx_hal::{buffer::Usage, device::Device as _, memory::Properties, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Buffer {
    driver: Driver,
    mem: Memory,
    ptr: Option<<_Backend as Backend>::Buffer>,
}

impl Buffer {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        usage: Usage,
        properties: Properties,
        len: u64,
    ) -> Self {
        let (buffer, mem) = {
            let device = driver.borrow();
            let mut buffer = unsafe { device.create_buffer(len as u64, usage) }.unwrap();

            #[cfg(debug_assertions)]
            unsafe {
                device.set_buffer_name(&mut buffer, name);
            }

            let requirements = unsafe { device.get_buffer_requirements(&buffer) };
            let mem_ty = Device::mem_ty(&device, requirements.type_mask, properties);
            let mem = Memory::new(Driver::clone(&driver), mem_ty, requirements.size);

            unsafe { device.bind_buffer_memory(&mem, 0, &mut buffer) }.unwrap();

            (buffer, mem)
        };

        Self {
            mem,
            ptr: Some(buffer),
            driver,
        }
    }

    pub fn mem(buf: &Self) -> &Memory {
        &buf.mem
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(debug_assertions)]
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
