use {
    super::Driver,
    gfx_hal::{device::Device, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct Fence {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::Fence>,
}

// TODO: Support naming in ctor

impl Fence {
    pub fn new(driver: Driver) -> Self {
        Self::new_signaled(driver, false)
    }

    pub fn new_signaled(driver: Driver, value: bool) -> Self {
        let fence = driver.borrow().create_fence(value).unwrap();

        Self {
            driver,
            ptr: Some(fence),
        }
    }

    pub fn reset(fence: &mut Self) {
        let device = fence.driver.borrow();

        unsafe { device.reset_fence(&fence) }.unwrap();
    }

    #[cfg(debug_assertions)]
    pub fn set_name(fence: &mut Self, name: &str) {
        let device = fence.driver.borrow();
        let ptr = fence.ptr.as_mut().unwrap();

        unsafe {
            device.set_fence_name(ptr, name);
        }
    }
}

impl AsMut<<_Backend as Backend>::Fence> for Fence {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Fence {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Fence> for Fence {
    fn as_ref(&self) -> &<_Backend as Backend>::Fence {
        &*self
    }
}

impl Deref for Fence {
    type Target = <_Backend as Backend>::Fence;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Fence {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device
                .wait_for_fence(&ptr, 0) // TODO: Double-check this zero usage
                .unwrap(); // TODO: Make a decision about ignoring this or just panic?
            device.destroy_fence(ptr);
        }
    }
}
