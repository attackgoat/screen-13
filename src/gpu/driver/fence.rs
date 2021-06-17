use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[cfg(debug_assertions)]
use std::time::Instant;

pub struct Fence(Option<<_Backend as Backend>::Fence>);

impl Fence {
    pub unsafe fn new(#[cfg(feature = "debug-names")] name: &str) -> Self {
        Self::new_signal(
            #[cfg(feature = "debug-names")]
            name,
            false,
        )
    }

    pub unsafe fn new_signal(#[cfg(feature = "debug-names")] name: &str, val: bool) -> Self {
        let ctor = || device().create_fence(val).unwrap();

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_fence_name(&mut ptr, name);

        Self(Some(ptr))
    }

    pub unsafe fn reset(fence: &mut Self) {
        device().reset_fence(fence).unwrap();
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(fence: &mut Self, name: &str) {
        let ptr = fence.0.as_mut().unwrap();
        device().set_fence_name(ptr, name);
    }

    pub unsafe fn status(fence: &Self) -> bool {
        let ptr = fence.0.as_ref().unwrap();

        // NOTE: We don't care if the device is lost!!
        device().get_fence_status(ptr).unwrap_or(true)
    }

    pub unsafe fn wait(fence: &Self) {
        // If the fence was ready or anything happened; just return as if we waited
        // otherwise we might hold up a drop function
        match device().wait_for_fence(fence, 1) {
            Err(_) => {
                warn!("Fence could not be waited on");

                return;
            }
            Ok(signaled) => {
                if signaled {
                    return;
                }

                warn!("Fence not signaled");
            }
        }

        #[cfg(feature = "no-gfx")]
        return;

        #[cfg(debug_assertions)]
        {
            let started = Instant::now();

            // TODO: Improve later
            for _ in 0..100 {
                if let Ok(true) | Err(_) = device().wait_for_fence(fence, 1_000_000) {
                    let elapsed = Instant::now() - started;
                    warn!("Graphics driver stalled! ({}ms)", elapsed.as_millis());

                    return;
                }
            }
        }

        #[cfg(feature = "use-gfx")]
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for Fence {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device()
                .wait_for_fence(&ptr, 0) // TODO: Double-check this zero usage
                .unwrap(); // TODO: Make a decision about ignoring this or just panic?
            device().destroy_fence(ptr);
        }
    }
}
