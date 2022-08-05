use {
    super::{Cache, Lease, Pool},
    crate::driver::{
        AccelerationStructure, AccelerationStructureInfo, AccelerationStructureInfoBuilder, Buffer,
        BufferInfo, BufferInfoBuilder, CommandBuffer, DescriptorPool, DescriptorPoolInfo, Device,
        DriverError, Image, ImageInfo, ImageInfoBuilder, QueueFamily, RenderPass, RenderPassInfo,
    },
    parking_lot::Mutex,
    std::{
        collections::{HashMap, VecDeque},
        fmt::Debug,
        sync::Arc,
    },
};

#[derive(Debug)]
pub struct HashPool {
    acceleration_structure_cache: HashMap<AccelerationStructureInfo, Cache<AccelerationStructure>>,
    buffer_cache: HashMap<BufferInfo, Cache<Buffer>>,
    command_buffer_cache: HashMap<QueueFamily, Cache<CommandBuffer>>,
    descriptor_pool_cache: HashMap<DescriptorPoolInfo, Cache<DescriptorPool>>,
    pub device: Arc<Device>,
    image_cache: HashMap<ImageInfo, Cache<Image>>,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass>>,
}

// TODO: Add some sort of manager features (like, I dunno, "Clear Some Memory For me")
impl HashPool {
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
                    let cache_ref = Arc::clone(cache);
                    let mut cache = cache.lock();

                    if cache.is_empty() || ![<can_lease_ $item:snake>](cache.front_mut().unwrap()) {
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

// Called by the lease macro
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

// Called by the lease macro
fn can_lease_render_pass(_: &mut RenderPass) -> bool {
    true
}

// Called by the lease macro
fn can_lease_descriptor_pool(_: &mut DescriptorPool) -> bool {
    true
}

lease!(QueueFamily => CommandBuffer);
lease!(RenderPassInfo => RenderPass);
lease!(DescriptorPoolInfo => DescriptorPool);

// Enable leasing items as above, but also using their info builder type for convenience
macro_rules! lease_builder {
    ($info:ident => $item:ident) => {
        lease!($info => $item);

        paste::paste! {
            // Called by the lease macro
            const fn [<can_lease_ $item:snake>]<T>(_: &T) -> bool {
                true
            }

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
