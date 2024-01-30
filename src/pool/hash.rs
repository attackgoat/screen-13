//! Pool which leases by exactly matching the information before creating new resources.

use {
    super::{can_lease_command_buffer, Cache, Lease, Pool, PoolInfo},
    crate::driver::{
        accel_struct::{AccelerationStructure, AccelerationStructureInfo},
        buffer::{Buffer, BufferInfo},
        device::Device,
        image::{Image, ImageInfo},
        CommandBuffer, CommandBufferInfo, DescriptorPool, DescriptorPoolInfo, DriverError,
        RenderPass, RenderPassInfo,
    },
    log::debug,
    parking_lot::Mutex,
    paste::paste,
    std::{
        collections::{HashMap, VecDeque},
        sync::Arc,
    },
};

/// A high-performance resource allocator.
///
/// # Bucket Strategy
///
/// The information for each lease request is the key for a `HashMap` of buckets. If no bucket
/// exists with the exact information provided a new bucket is created.
///
/// In practice this means that for a [`PoolInfo::image_capacity`] of `4`, requests for a 1024x1024
/// image with certain attributes will store a maximum of `4` such images. Requests for any image
/// having a different size or attributes will store an additional maximum of `4` images.
///
/// # Memory Management
///
/// If requests for varying resources is common [`HashPool::clear_images_by_info`] and other memory
/// management functions are nessecery in order to avoid using all available device memory.
#[derive(Debug)]
pub struct HashPool {
    acceleration_structure_cache: HashMap<AccelerationStructureInfo, Cache<AccelerationStructure>>,
    buffer_cache: HashMap<BufferInfo, Cache<Buffer>>,
    command_buffer_cache: HashMap<u32, Cache<CommandBuffer>>,
    descriptor_pool_cache: HashMap<DescriptorPoolInfo, Cache<DescriptorPool>>,
    device: Arc<Device>,
    image_cache: HashMap<ImageInfo, Cache<Image>>,
    info: PoolInfo,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass>>,
}

impl HashPool {
    /// Constructs a new `HashPool`.
    pub fn new(device: &Arc<Device>) -> Self {
        Self::with_capacity(device, PoolInfo::default())
    }

    /// Constructs a new `HashPool` with the given capacity information.
    pub fn with_capacity(device: &Arc<Device>, info: impl Into<PoolInfo>) -> Self {
        let info: PoolInfo = info.into();
        let device = Arc::clone(device);

        Self {
            acceleration_structure_cache: Default::default(),
            buffer_cache: Default::default(),
            command_buffer_cache: Default::default(),
            descriptor_pool_cache: Default::default(),
            device,
            image_cache: Default::default(),
            info,
            render_pass_cache: Default::default(),
        }
    }

    /// Clears the pool, removing all resources.
    pub fn clear(&mut self) {
        self.clear_accel_structs();
        self.clear_buffers();
        self.clear_images();
    }
}

macro_rules! resource_mgmt_fns {
    ($fn_plural:literal, $doc_singular:literal, $ty:ty, $field:ident) => {
        paste! {
            impl HashPool {
                #[doc = "Clears the pool of " $doc_singular " resources."]
                pub fn [<clear_ $fn_plural>](&mut self) {
                    self.$field.clear();
                }

                #[doc = "Clears the pool of all " $doc_singular " resources matching the given
information."]
                pub fn [<clear_ $fn_plural _by_info>](
                    &mut self,
                    info: impl Into<$ty>,
                ) {
                    self.$field.remove(&info.into());
                }

                #[doc = "Retains only the " $doc_singular " resources specified by the predicate.\n
\nIn other words, remove all " $doc_singular " resources for which `f(" $ty ")` returns `false`.\n
\n"]
                /// The elements are visited in unsorted (and unspecified) order.
                ///
                /// # Performance
                ///
                /// Provides the same performance guarantees as
                /// [`HashMap::retain`](HashMap::retain).
                pub fn [<retain_ $fn_plural>]<F>(&mut self, mut f: F)
                where
                    F: FnMut($ty) -> bool,
                {
                    self.$field.retain(|&info, _| f(info))
                }
            }
        }
    };
}

resource_mgmt_fns!(
    "accel_structs",
    "acceleration structure",
    AccelerationStructureInfo,
    acceleration_structure_cache
);
resource_mgmt_fns!("buffers", "buffer", BufferInfo, buffer_cache);
resource_mgmt_fns!("images", "image", ImageInfo, image_cache);

impl Pool<CommandBufferInfo, CommandBuffer> for HashPool {
    #[profiling::function]
    fn lease(&mut self, info: CommandBufferInfo) -> Result<Lease<CommandBuffer>, DriverError> {
        let cache = self
            .command_buffer_cache
            .entry(info.queue_family_index)
            .or_insert_with(PoolInfo::default_cache);
        let mut item = cache
            .lock()
            .pop_front()
            .filter(can_lease_command_buffer)
            .map(Ok)
            .unwrap_or_else(|| {
                debug!("Creating new {}", stringify!(CommandBuffer));

                CommandBuffer::create(&self.device, info)
            })?;

        // Drop anything we were holding from the last submission
        CommandBuffer::drop_fenced(&mut item);

        Ok(Lease::new(Arc::downgrade(cache), item))
    }
}

impl Pool<DescriptorPoolInfo, DescriptorPool> for HashPool {
    #[profiling::function]
    fn lease(&mut self, info: DescriptorPoolInfo) -> Result<Lease<DescriptorPool>, DriverError> {
        let cache = self
            .descriptor_pool_cache
            .entry(info.clone())
            .or_insert_with(PoolInfo::default_cache);
        let item = cache.lock().pop_front().map(Ok).unwrap_or_else(|| {
            debug!("Creating new {}", stringify!(DescriptorPool));

            DescriptorPool::create(&self.device, info)
        })?;

        Ok(Lease::new(Arc::downgrade(cache), item))
    }
}

impl Pool<RenderPassInfo, RenderPass> for HashPool {
    #[profiling::function]
    fn lease(&mut self, info: RenderPassInfo) -> Result<Lease<RenderPass>, DriverError> {
        let cache = if let Some(cache) = self.render_pass_cache.get(&info) {
            cache
        } else {
            // We tried to get the cache first in order to avoid this clone
            self.render_pass_cache
                .entry(info.clone())
                .or_insert_with(PoolInfo::default_cache)
        };
        let item = cache.lock().pop_front().map(Ok).unwrap_or_else(|| {
            debug!("Creating new {}", stringify!(RenderPass));

            RenderPass::create(&self.device, info)
        })?;

        Ok(Lease::new(Arc::downgrade(cache), item))
    }
}

// Enable leasing items using their basic info
macro_rules! lease {
    ($info:ident => $item:ident, $capacity:ident) => {
        paste::paste! {
            impl Pool<$info, $item> for HashPool {
                #[profiling::function]
                fn lease(&mut self, info: $info) -> Result<Lease<$item>, DriverError> {
                    let cache = self.[<$item:snake _cache>].entry(info.clone())
                        .or_insert_with(|| {
                            Cache::new(Mutex::new(VecDeque::with_capacity(self.info.$capacity)))
                        });
                    let item = cache.lock().pop_front().map(Ok).unwrap_or_else(|| {
                        debug!("Creating new {}", stringify!($item));

                        $item::create(&self.device, info)
                    })?;

                    Ok(Lease::new(Arc::downgrade(cache), item))
                }
            }
        }
    };
}

lease!(AccelerationStructureInfo => AccelerationStructure, accel_struct_capacity);
lease!(BufferInfo => Buffer, buffer_capacity);
lease!(ImageInfo => Image, image_capacity);
