use {
    super::Device,
    gfx_hal::{device::Device as _, Backend, MemoryTypeId},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Memory {
    device: Device,
    ptr: Option<<_Backend as Backend>::Memory>,
    size: u64,
}

impl Memory {
    pub fn new<M: Into<MemoryTypeId>>(device: Device, mem_ty: M, size: u64) -> Self {
        #[cfg(debug_assertions)]
        assert_ne!(size, 0);

        let mem = {
            let mem_ty = mem_ty.into();

            unsafe { device.allocate_memory(mem_ty, size).unwrap() }
        };

        Self {
            device,
            ptr: Some(mem),
            size,
        }
    }

    pub fn size(mem: &Self) -> u64 {
        mem.size
    }
}

impl AsMut<<_Backend as Backend>::Memory> for Memory {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Memory {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Memory> for Memory {
    fn as_ref(&self) -> &<_Backend as Backend>::Memory {
        &*self
    }
}

impl Deref for Memory {
    type Target = <_Backend as Backend>::Memory;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.free_memory(ptr);
        }
    }
}
