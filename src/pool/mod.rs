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

pub mod fifo;
pub mod hash;
pub mod lazy;

use {
    crate::driver::{
        accel_struct::{
            AccelerationStructure, AccelerationStructureInfo, AccelerationStructureInfoBuilder,
        },
        buffer::{Buffer, BufferInfo, BufferInfoBuilder},
        image::{Image, ImageInfo, ImageInfoBuilder},
        CommandBuffer, DriverError,
    },
    derive_builder::{Builder, UninitializedFieldError},
    parking_lot::Mutex,
    std::{
        collections::VecDeque,
        fmt::Debug,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        sync::{Arc, Weak},
        thread::panicking,
    },
};

type Cache<T> = Arc<Mutex<VecDeque<T>>>;
type CacheRef<T> = Weak<Mutex<VecDeque<T>>>;

fn can_lease_command_buffer(cmd_buf: &CommandBuffer) -> bool {
    unsafe {
        // Don't lease this command buffer if it is unsignalled; we'll create a new one
        // and wait for this, and those behind it, to signal.
        cmd_buf
            .device
            .get_fence_status(cmd_buf.fence)
            .unwrap_or_default()
    }
}

/// Holds a leased resource and implements `Drop` in order to return the resource.
///
/// This simple wrapper type implements only the `AsRef`, `AsMut`, `Deref` and `DerefMut` traits
/// and provides no other functionality. A freshly leased resource is guaranteed to have no other
/// owners and may be mutably accessed.
#[derive(Debug)]
pub struct Lease<T> {
    cache_ref: CacheRef<T>,
    item: ManuallyDrop<T>,
}

impl<T> Lease<T> {
    fn new(cache_ref: CacheRef<T>, item: T) -> Self {
        Self {
            cache_ref,
            item: ManuallyDrop::new(item),
        }
    }
}

impl<T> AsRef<T> for Lease<T> {
    fn as_ref(&self) -> &T {
        &self.item
    }
}

impl<T> AsMut<T> for Lease<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.item
    }
}

impl<T> Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T> DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        // If the pool cache has been dropped we must manually drop the item, otherwise it goes back
        // into the pool.
        if let Some(cache) = self.cache_ref.upgrade() {
            let mut cache = cache.lock();

            if cache.len() >= cache.capacity() {
                cache.pop_front();
            }

            cache.push_back(unsafe { ManuallyDrop::take(&mut self.item) });
        } else {
            unsafe {
                ManuallyDrop::drop(&mut self.item);
            }
        }
    }
}

/// Allows leasing of resources using driver information structures.
pub trait Pool<I, T> {
    /// Lease a resource.
    fn lease(&mut self, info: I) -> Result<Lease<T>, DriverError>;
}

// Enable leasing items using their info builder type for convenience
macro_rules! lease_builder {
    ($info:ident => $item:ident) => {
        paste::paste! {
            impl<T> Pool<[<$info Builder>], $item> for T where T: Pool<$info, $item> {
                fn lease(&mut self, builder: [<$info Builder>]) -> Result<Lease<$item>, DriverError> {
                    let info = builder.build();

                    self.lease(info)
                }
            }
        }
    };
}

lease_builder!(AccelerationStructureInfo => AccelerationStructure);
lease_builder!(BufferInfo => Buffer);
lease_builder!(ImageInfo => Image);

/// TBD
#[derive(Builder, Clone, Copy, Debug)]
#[builder(
    build_fn(private, name = "fallible_build", error = "PoolInfoBuilderError"),
    derive(Debug),
    pattern = "owned"
)]
pub struct PoolInfo {
    /// TBD
    #[builder(default = "PoolInfo::DEFAULT_RESOURCE_CAPACITY", setter(strip_option))]
    pub accel_struct_capacity: usize,

    /// TBD
    #[builder(default = "PoolInfo::DEFAULT_RESOURCE_CAPACITY", setter(strip_option))]
    pub buffer_capacity: usize,

    /// TBD
    #[builder(default = "PoolInfo::DEFAULT_RESOURCE_CAPACITY", setter(strip_option))]
    pub image_capacity: usize,
}

impl PoolInfo {
    /// TBD
    pub const DEFAULT_RESOURCE_CAPACITY: usize = 4;

    /// TBD
    pub const fn with_capacity(resource_capacity: usize) -> Self {
        Self {
            accel_struct_capacity: resource_capacity,
            buffer_capacity: resource_capacity,
            image_capacity: resource_capacity,
        }
    }

    fn default_cache<T>() -> Cache<T> {
        Cache::new(Mutex::new(VecDeque::with_capacity(
            Self::DEFAULT_RESOURCE_CAPACITY,
        )))
    }

    fn explicit_cache<T>(capacity: usize) -> Cache<T> {
        Cache::new(Mutex::new(VecDeque::with_capacity(capacity)))
    }
}

impl Default for PoolInfo {
    fn default() -> Self {
        PoolInfoBuilder::default().into()
    }
}

impl From<PoolInfoBuilder> for PoolInfo {
    fn from(info: PoolInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl PoolInfoBuilder {
    /// Builds a new `PoolInfo`.
    pub fn build(self) -> PoolInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
struct PoolInfoBuilderError;

impl From<UninitializedFieldError> for PoolInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}
