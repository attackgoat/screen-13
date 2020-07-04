use {
    super::Driver,
    gfx_hal::{device::Device, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct Fence {
    driver: Driver,
    fence: Option<<_Backend as Backend>::Fence>,
}

impl Fence {
    pub fn new(driver: Driver) -> Self {
        Self::new_signaled(driver, false)
    }

    pub fn new_signaled(driver: Driver, value: bool) -> Self {
        let fence = driver.borrow().create_fence(value).unwrap();

        Self {
            driver,
            fence: Some(fence),
        }
    }

    pub fn reset(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .reset_fence(self.fence.as_mut().unwrap())
                .unwrap();
        }
    }
}

impl AsRef<<_Backend as Backend>::Fence> for Fence {
    fn as_ref(&self) -> &<_Backend as Backend>::Fence {
        self.fence.as_ref().unwrap()
    }
}

impl Deref for Fence {
    type Target = <_Backend as Backend>::Fence;

    fn deref(&self) -> &Self::Target {
        self.fence.as_ref().unwrap()
    }
}

impl DerefMut for Fence {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.fence.as_mut().unwrap()
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .wait_for_fence(self.fence.as_ref().unwrap(), 0)
                .unwrap();
            self.driver
                .borrow()
                .destroy_fence(self.fence.take().unwrap());
        }
    }
}
