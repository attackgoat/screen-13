use {
    super::Driver,
    gfx_hal::{device::Device, Backend, MemoryTypeId},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct Memory {
    driver: Driver,
    mem: Option<<_Backend as Backend>::Memory>,
    size: u64,
}

impl Memory {
    pub fn new<M: Into<MemoryTypeId>>(driver: Driver, mem_type: M, size: u64) -> Self {
        #[cfg(debug_assertions)]
        assert_ne!(size, 0);

        let mem = unsafe {
            driver
                .borrow()
                .allocate_memory(mem_type.into(), size)
                .unwrap()
        };

        Self {
            driver,
            mem: Some(mem),
            size,
        }
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

impl Deref for Memory {
    type Target = <_Backend as Backend>::Memory;

    fn deref(&self) -> &Self::Target {
        self.mem.as_ref().unwrap()
    }
}

impl DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mem.as_mut().unwrap()
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        let mem = self.mem.take().unwrap();

        unsafe {
            self.driver.borrow().free_memory(mem);
        }
    }
}
