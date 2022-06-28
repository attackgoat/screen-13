use {
    super::{Cache, Contract, Lease, Pool},
    crate::driver::{
        AccelerationStructure, AccelerationStructureInfo, AccelerationStructureInfoBuilder, Buffer,
        BufferInfo, BufferInfoBuilder, CommandBuffer, DescriptorPool, DescriptorPoolInfo,
        DescriptorPoolInfoBuilder, Device, DriverError, Image, ImageInfo, ImageInfoBuilder,
        QueueFamily, RenderPass, RenderPassInfo, RenderPassInfoBuilder,
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

impl Pool<DescriptorPoolInfo, DescriptorPool> for HashPool {
    fn lease(
        &mut self,
        contract: DescriptorPoolInfo,
    ) -> Result<Lease<DescriptorPool>, DriverError> {
        todo!();
    }
}

impl Pool<RenderPassInfo, RenderPass> for HashPool {
    fn lease(&mut self, contract: RenderPassInfo) -> Result<Lease<RenderPass>, DriverError> {
        todo!();
    }
}

impl Pool<QueueFamily, CommandBuffer> for HashPool {
    fn lease(&mut self, contract: QueueFamily) -> Result<Lease<CommandBuffer>, DriverError> {
        todo!();
    }
}

/*
// Enable the basic leasing of items
macro_rules! lease {
    ($src:ident => $dst:ident) => {
        impl Contract for $src {
            type Term = $dst;
        }

        paste::paste! {
            impl Pool<Lease<$dst>, HashPool> for $src {
                fn lease(self, pool: &mut HashPool) -> Result<Lease<$dst>, DriverError> {
                    let cache = pool.[<$dst:snake _cache>].entry(self.clone())
                        .or_insert_with(|| {
                            Arc::new(Mutex::new(VecDeque::new()))
                        });
                    let cache_ref = Arc::clone(cache);
                    let mut cache = cache.lock();

                    if cache.is_empty() || ![<can_lease_ $dst:snake>](cache.front_mut().unwrap()) {
                        // Calls the function defined in the other macros
                        let item = [<create_ $dst:snake>](&pool.device, self)?;

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

// Enable leasing items using their basic info as the entire request
macro_rules! lease_info {
    ($src:ident => $dst:ident) => {
        lease!($src => $dst);

        paste::paste! {
            // Called by the lease macro
            fn [<create_ $dst:snake>](
                device: &Arc<Device>,
                info: $src
            ) -> Result<$dst, DriverError> {
                $dst::create(device, info)
            }
        }
    };
}

lease_info!(QueueFamily => CommandBuffer);

// Used by macro invocation, above
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

// Enable leasing items as above, but also using their info builder type for convenience
macro_rules! lease_info_builder {
    ($src:ident => $dst:ident) => {
        lease_info!($src => $dst);

        paste::paste! {
            // Called by the lease macro, via the lease_info macro
            const fn [<can_lease_ $dst:snake>]<T>(_: &T) -> bool {
                true
            }

            impl Contract for [<$src Builder>] {
                type Term = $dst;
            }

            impl Pool<Lease<$dst>, HashPool> for [<$src Builder>] {
                fn lease(self, pool: &mut HashPool) -> Result<Lease<$dst>, DriverError> {
                    let info = self.build();

                    // We will unwrap the info builder - it may panic!
                    assert!(info.is_ok(), "Invalid pool resource info: {:#?}", info);

                    info.unwrap().lease(pool)
                }
            }
        }
    };
}

lease_info_builder!(RenderPassInfo => RenderPass);

macro_rules! lease_info_binding {
    ($src:ident => $dst:ident) => {
        paste::paste! {
            impl Contract for $src {
                type Term = $dst;
            }

            paste::paste! {
                impl Pool<Lease<$dst>, HashPool> for $src {
                    fn lease(self, pool: &mut HashPool) -> Result<Lease<$dst>, DriverError> {
                        let cache = pool.[<$dst:snake _cache>].entry(self.clone())
                            .or_insert_with(|| {
                                Arc::new(Mutex::new(VecDeque::new()))
                            });
                        let cache_ref = Arc::clone(cache);
                        let mut cache = cache.lock();

                        if cache.is_empty() || ![<can_lease_ $dst:snake>](cache.front_mut().unwrap()) {
                            // Calls the function defined in the other macros
                            let item = [<create_ $dst:snake>](&pool.device, self)?;

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

            // Called by the lease macro
            fn [<create_ $dst:snake>](
                device: &Arc<Device>,
                info: $src
            ) -> Result<$dst, DriverError> {
                $dst::create(device, info)
            }

            // Called by the lease macro
            fn [<can_lease_ $dst:snake>]<T>(_: &mut T) -> bool {
                true
            }

            impl Contract for [<$src Builder>] {
                type Term = $dst;
            }

            impl Pool<Lease<$dst>, HashPool> for [<$src Builder>] {
                fn lease(self, pool: &mut HashPool) -> Result<Lease<$dst>, DriverError> {
                    self.build().lease(pool)
                }
            }
        }
    };
}

lease_info_binding!(AccelerationStructureInfo => AccelerationStructure);
lease_info_binding!(BufferInfo => Buffer);
lease_info_binding!(ImageInfo => Image);
lease_info_binding!(DescriptorPoolInfo => DescriptorPool);
 */
