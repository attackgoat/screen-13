//! Pool which leases by exactly matching the information before creating new resources.
//!
//! The information for each lease request is placed into a `HashMap`. If no resources exist for
//! the exact information provided then a new resource is created and returned.

use {
    super::{Cache, Lease, Pool},
    crate::driver::{
        accel_struct::{
            AccelerationStructure, AccelerationStructureInfo, AccelerationStructureInfoBuilder,
        },
        buffer::{Buffer, BufferInfo, BufferInfoBuilder},
        device::Device,
        image::{Image, ImageInfo, ImageInfoBuilder},
        CommandBuffer, CommandBufferInfo, DescriptorPool, DescriptorPoolInfo, DriverError,
        RenderPass, RenderPassInfo,
    },
    parking_lot::Mutex,
    std::{
        collections::{HashMap, VecDeque},
        fmt::Debug,
        sync::Arc,
    },
};

/// A high-performance resource allocator.
#[derive(Debug)]
pub struct HashPool {
    acceleration_structure_cache: HashMap<AccelerationStructureInfo, Cache<AccelerationStructure>>,
    buffer_cache: HashMap<BufferInfo, Cache<Buffer>>,
    command_buffer_cache: Cache<CommandBuffer>,
    descriptor_pool_cache: HashMap<DescriptorPoolInfo, Cache<DescriptorPool>>,
    device: Arc<Device>,
    image_cache: HashMap<ImageInfo, Cache<Image>>,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass>>,
}

// TODO: Add some sort of manager features (like, I dunno, "Clear Some Memory For me")
impl HashPool {
    /// Constructs a new `HashPool`.
    pub fn new(device: &Arc<Device>) -> Self {
        let device = Arc::clone(device);

        Self {
            acceleration_structure_cache: Default::default(),
            buffer_cache: Default::default(),
            command_buffer_cache: Default::default(),
            descriptor_pool_cache: Default::default(),
            device,
            image_cache: Default::default(),
            render_pass_cache: Default::default(),
        }
    }
}

impl HashPool {
    fn can_lease_command_buffer(cmd_buf: &mut CommandBuffer) -> bool {
        let can_lease = unsafe {
            // Don't lease this command buffer if it is unsignalled; we'll create a new one
            // and wait for this, and those behind it, to signal.
            cmd_buf
                .device
                .get_fence_status(cmd_buf.fence)
                .unwrap_or_default()
        };

        if can_lease {
            // Drop anything we were holding from the last submission
            CommandBuffer::drop_fenced(cmd_buf);
        }

        can_lease
    }
}

impl Pool<CommandBufferInfo, CommandBuffer> for HashPool {
    fn lease(&mut self, info: CommandBufferInfo) -> Result<Lease<CommandBuffer>, DriverError> {
        let cache_ref = Arc::downgrade(&self.command_buffer_cache);
        let mut cache = self.command_buffer_cache.lock();

        if cache.is_empty() || !Self::can_lease_command_buffer(cache.front_mut().unwrap()) {
            let item = CommandBuffer::create(&self.device, info)?;

            return Ok(Lease {
                cache: Some(cache_ref),
                item: Some(item),
            });
        }

        Ok(Lease {
            cache: Some(cache_ref),
            item: cache.pop_front(),
        })
    }
}

// Enable leasing items using their basic info
macro_rules! lease {
    ($info:ident => $item:ident) => {
        paste::paste! {
            impl Pool<$info, $item> for HashPool {
                fn lease(&mut self, info: $info) -> Result<Lease<$item>, DriverError> {
                    let cache = self.[<$item:snake _cache>].entry(info.clone())
                        .or_insert_with(|| {
                            Arc::new(Mutex::new(VecDeque::new()))
                        });
                    let cache_ref = Arc::downgrade(cache);
                    let mut cache = cache.lock();

                    if cache.is_empty() {
                        let item = $item::create(&self.device, info)?;

                        return Ok(Lease {
                            cache: Some(cache_ref),
                            item: Some(item),
                        });
                    }

                    Ok(Lease {
                        cache: Some(cache_ref),
                        item: cache.pop_front(),
                    })
                }
            }
        }
    };
}

lease!(RenderPassInfo => RenderPass);
lease!(DescriptorPoolInfo => DescriptorPool);

// Enable leasing items as above, but also using their info builder type for convenience
macro_rules! lease_builder {
    ($info:ident => $item:ident) => {
        lease!($info => $item);

        paste::paste! {
            impl Pool<[<$info Builder>], $item> for HashPool {
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
