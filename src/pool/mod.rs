//! Resource leasing and pooling types.
//!
//! _Screen 13_ provides caching for acceleration structure, buffer, and image resources which may
//! be leased from configurable pools using their corresponding information structure. Most programs
//! will do fine with a single `LazyPool`.
//!
//! Leased resources may be bound directly to a render graph and used in the same manner as regular
//! resources. After rendering has finished, the leased resources will return to the pool for reuse.
//!
//! # Examples
//!
//! Leasing an image:
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use ash::vk;
//! # use screen_13::driver::DriverError;
//! # use screen_13::driver::device::{Device, DeviceInfo};
//! # use screen_13::driver::image::{ImageInfo};
//! # use screen_13::pool::{Pool};
//! # use screen_13::pool::lazy::{LazyPool};
//! # fn main() -> Result<(), DriverError> {
//! # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
//! let mut pool = LazyPool::new(&device);
//!
//! let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 8, 8, vk::ImageUsageFlags::STORAGE);
//! let my_image = pool.lease(info)?;
//!
//! assert!(my_image.info.usage.contains(vk::ImageUsageFlags::STORAGE));
//! # Ok(()) }
//! ```
//!
//! # When Should You Use Which Pool?
//!
//! These are fairly high-level break-downs of when each pool should be considered. You may need
//! to investigate each type of pool individually or write your own implementation to provide the
//! absolute best fit for your purpose.
//!
//! ### Use a `LazyPool` when:
//! * Memory usage is most important
//! * Resources have different attributes each frame
//!
//! ### Use a `HashPool` when:
//! * Processor usage is most important
//! * Resources have consistent attributes each frame

pub mod hash;
pub mod lazy;

use {
    crate::driver::DriverError,
    parking_lot::Mutex,
    std::{
        collections::VecDeque,
        fmt::Debug,
        ops::{Deref, DerefMut},
        sync::{Arc, Weak},
        thread::panicking,
    },
};

type Cache<T> = Arc<Mutex<VecDeque<T>>>;
type CacheRef<T> = Weak<Mutex<VecDeque<T>>>;

/// Holds a leased resource and implements `Drop` in order to return the resource.
///
/// This simple wrapper type implements only the `AsRef`, `AsMut`, `Deref` and `DerefMut` traits
/// and provides no other functionality. A freshly leased resource is guaranteed to have no other
/// owners and may be mutably accessed.
#[derive(Debug)]
pub struct Lease<T> {
    cache: Option<CacheRef<T>>,
    item: Option<T>,
}

impl<T> AsRef<T> for Lease<T> {
    fn as_ref(&self) -> &T {
        self.item.as_ref().unwrap()
    }
}

impl<T> AsMut<T> for Lease<T> {
    fn as_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }
}

impl<T> Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T> DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        if let Some(cache) = self.cache.as_ref() {
            if let Some(cache) = cache.upgrade() {
                cache.lock().push_back(self.item.take().unwrap());
            }
        }
    }
}

/// Allows leasing of resources using driver information structures.
pub trait Pool<I, T> {
    /// Lease a resource.
    fn lease(&mut self, info: I) -> Result<Lease<T>, DriverError>;
}
