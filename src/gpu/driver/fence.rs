use {
    super::Driver,
    gfx_hal::{device::Device, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[cfg(debug_assertions)]
use std::time::Instant;

pub struct Fence {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::Fence>,
}

// TODO: Support naming in ctor

impl Fence {
    pub fn new(#[cfg(debug_assertions)] name: &str, driver: Driver) -> Self {
        Self::with_signal(
            #[cfg(debug_assertions)]
            name,
            driver,
            false,
        )
    }

    pub fn with_signal(#[cfg(debug_assertions)] name: &str, driver: Driver, value: bool) -> Self {
        let fence = {
            let device = driver.borrow();
            let ctor = || device.create_fence(value).unwrap();

            #[cfg(debug_assertions)]
            let mut fence = ctor();

            #[cfg(not(debug_assertions))]
            let fence = ctor();

            #[cfg(debug_assertions)]
            unsafe {
                device.set_fence_name(&mut fence, name);
            }

            fence
        };

        Self {
            driver,
            ptr: Some(fence),
        }
    }

    pub fn reset(fence: &mut Self) {
        let device = fence.driver.borrow();

        unsafe { device.reset_fence(&fence) }.unwrap();
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(debug_assertions)]
    pub fn set_name(fence: &mut Self, name: &str) {
        let device = fence.driver.borrow();
        let ptr = fence.ptr.as_mut().unwrap();

        unsafe {
            device.set_fence_name(ptr, name);
        }
    }

    pub fn wait(fence: &Self) {
        let device = fence.driver.borrow();

        unsafe {
            // If the fence was ready or anything happened; just return as if we waited
            // otherwise we might hold up a drop function
            if let Ok(true) | Err(_) = device.wait_for_fence(fence, 0) {
                return;
            }

            #[cfg(debug_assertions)]
            {
                let started = Instant::now();

                // TODO: Improve later
                for _ in 0..100 {
                    if let Ok(true) | Err(_) = device.wait_for_fence(fence, 1_000_000) {
                        let elapsed = Instant::now() - started;
                        warn!("Graphics driver stalled! ({}ms)", elapsed.as_millis());

                        return;
                    }
                }
            }
        }

        panic!("Graphics driver stalled!");
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
