use {
    super::{Driver, Memory, PhysicalDevice},
    gfx_hal::{buffer::Usage, device::Device, memory::Properties, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct Buffer {
    mem: Memory,
    buffer: Option<<_Backend as Backend>::Buffer>,
    driver: Driver,
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
            let mem_type = device.get_mem_type(requirements.type_mask, properties);
            let mem = Memory::new(Driver::clone(&driver), mem_type, requirements.size);

            unsafe {
                device.bind_buffer_memory(&mem, 0, &mut buffer).unwrap();
            }

            (buffer, mem)
        };

        Self {
            mem,
            buffer: Some(buffer),
            driver,
        }
    }

    pub fn mem(&self) -> &Memory {
        &self.mem
    }
}

impl AsRef<<_Backend as Backend>::Buffer> for Buffer {
    fn as_ref(&self) -> &<_Backend as Backend>::Buffer {
        self.buffer.as_ref().unwrap()
    }
}

impl Deref for Buffer {
    type Target = <_Backend as Backend>::Buffer;

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .destroy_buffer(self.buffer.take().unwrap());
        }
    }
}
