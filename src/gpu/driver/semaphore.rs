use {
    super::Driver,
    gfx_hal::{device::Device, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct Semaphore {
    driver: Driver,
    semaphore: Option<<_Backend as Backend>::Semaphore>,
}

impl Semaphore {
    pub fn new(driver: Driver) -> Self {
        let semaphore = driver.borrow().create_semaphore().unwrap();

        Self {
            driver,
            semaphore: Some(semaphore),
        }
    }
}

impl AsRef<<_Backend as Backend>::Semaphore> for Semaphore {
    fn as_ref(&self) -> &<_Backend as Backend>::Semaphore {
        self.semaphore.as_ref().unwrap()
    }
}

impl Deref for Semaphore {
    type Target = <_Backend as Backend>::Semaphore;

    fn deref(&self) -> &Self::Target {
        self.semaphore.as_ref().unwrap()
    }
}

impl DerefMut for Semaphore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.semaphore.as_mut().unwrap()
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .destroy_semaphore(self.semaphore.take().unwrap());
        }
    }
}
