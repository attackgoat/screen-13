use {
    super::{Cache, Lease, Pool},
    crate::driver::{
        accel_struct::{
            AccelerationStructure, AccelerationStructureInfo, AccelerationStructureInfoBuilder,
        },
        buffer::{Buffer, BufferInfo, BufferInfoBuilder},
        image::{Image, ImageInfo, ImageInfoBuilder, ImageType, SampleCount},
        CommandBuffer, DescriptorPool, DescriptorPoolInfo, Device, DriverError, QueueFamily,
        RenderPass, RenderPassInfo,
    },
    ash::vk,
    parking_lot::Mutex,
    std::{
        collections::{HashMap, VecDeque},
        fmt::Debug,
        sync::Arc,
    },
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct ImageKey {
    array_elements: u32,
    depth: u32,
    fmt: vk::Format,
    height: u32,
    linear_tiling: bool,
    mip_level_count: u32,
    sample_count: SampleCount,
    ty: ImageType,
    width: u32,
}

#[derive(Debug)]
pub struct LazyPool {
    acceleration_structure_cache:
        HashMap<vk::AccelerationStructureTypeKHR, Cache<AccelerationStructure>>,
    buffer_cache: HashMap<bool, Cache<Buffer>>,
    command_buffer_cache: HashMap<u32, Cache<CommandBuffer>>,
    descriptor_pool_cache: Cache<DescriptorPool>,
    pub device: Arc<Device>,
    image_cache: HashMap<ImageKey, Cache<Image>>,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass>>,
}

// TODO: Add some sort of manager features (like, I dunno, "Clear Some Memory For me")
impl LazyPool {
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

impl Pool<AccelerationStructureInfo, AccelerationStructure> for LazyPool {
    fn lease(
        &mut self,
        info: AccelerationStructureInfo,
    ) -> Result<Lease<AccelerationStructure>, DriverError> {
        let acceleration_structure_cache = self
            .acceleration_structure_cache
            .entry(info.ty)
            .or_default();
        let cache_ref = Arc::clone(acceleration_structure_cache);
        let mut cache = acceleration_structure_cache.lock();

        if cache.is_empty() {
            let item = AccelerationStructure::create(&self.device, info)?;

            return Ok(Lease {
                cache: Some(cache_ref),
                item: Some(item),
            });
        }

        // Look for a compatible acceleration structure (big enough)
        for idx in 0..cache.len() {
            let item = &cache[idx];
            if item.info.size >= info.size {
                let item = cache.remove(idx).unwrap();

                return Ok(Lease {
                    cache: Some(cache_ref),
                    item: Some(item),
                });
            }
        }

        let item = AccelerationStructure::create(&self.device, info)?;

        Ok(Lease {
            cache: Some(cache_ref),
            item: Some(item),
        })
    }
}

impl Pool<AccelerationStructureInfoBuilder, AccelerationStructure> for LazyPool {
    fn lease(
        &mut self,
        info: AccelerationStructureInfoBuilder,
    ) -> Result<Lease<AccelerationStructure>, DriverError> {
        self.lease(info.build())
    }
}

impl Pool<BufferInfo, Buffer> for LazyPool {
    fn lease(&mut self, info: BufferInfo) -> Result<Lease<Buffer>, DriverError> {
        let buffer_cache = self.buffer_cache.entry(info.can_map).or_default();
        let cache_ref = Arc::clone(buffer_cache);
        let mut cache = buffer_cache.lock();

        if cache.is_empty() {
            let item = Buffer::create(&self.device, info)?;

            return Ok(Lease {
                cache: Some(cache_ref),
                item: Some(item),
            });
        }

        // Look for a compatible buffer (same mapping mode, big enough, superset of usage flags)
        for idx in 0..cache.len() {
            let item = &cache[idx];
            if item.info.can_map == info.can_map
                && item.info.size >= info.size
                && item.info.usage.contains(info.usage)
            {
                let item = cache.remove(idx).unwrap();

                return Ok(Lease {
                    cache: Some(cache_ref),
                    item: Some(item),
                });
            }
        }

        let item = Buffer::create(&self.device, info)?;

        Ok(Lease {
            cache: Some(cache_ref),
            item: Some(item),
        })
    }
}

impl Pool<BufferInfoBuilder, Buffer> for LazyPool {
    fn lease(&mut self, info: BufferInfoBuilder) -> Result<Lease<Buffer>, DriverError> {
        self.lease(info.build())
    }
}

impl Pool<DescriptorPoolInfo, DescriptorPool> for LazyPool {
    fn lease(&mut self, info: DescriptorPoolInfo) -> Result<Lease<DescriptorPool>, DriverError> {
        let cache_ref = Arc::clone(&self.descriptor_pool_cache);
        let mut cache = self.descriptor_pool_cache.lock();

        if cache.is_empty() {
            let item = DescriptorPool::create(&self.device, info)?;

            return Ok(Lease {
                cache: Some(cache_ref),
                item: Some(item),
            });
        }

        // Look for a compatible descriptor pool (has enough sets and descriptors)
        for idx in 0..cache.len() {
            let item = &cache[idx];
            if item.info.max_sets >= info.max_sets
                && item.info.acceleration_structure_count >= info.acceleration_structure_count
                && item.info.combined_image_sampler_count >= info.combined_image_sampler_count
                && item.info.input_attachment_count >= info.input_attachment_count
                && item.info.sampled_image_count >= info.sampled_image_count
                && item.info.storage_buffer_count >= info.storage_buffer_count
                && item.info.storage_buffer_dynamic_count >= info.storage_buffer_dynamic_count
                && item.info.storage_image_count >= info.storage_image_count
                && item.info.storage_texel_buffer_count >= info.storage_texel_buffer_count
                && item.info.uniform_buffer_count >= info.uniform_buffer_count
                && item.info.uniform_buffer_dynamic_count >= info.uniform_buffer_dynamic_count
                && item.info.uniform_texel_buffer_count >= info.uniform_texel_buffer_count
            {
                let item = cache.remove(idx).unwrap();

                return Ok(Lease {
                    cache: Some(cache_ref),
                    item: Some(item),
                });
            }
        }

        let item = DescriptorPool::create(&self.device, info)?;

        Ok(Lease {
            cache: Some(cache_ref),
            item: Some(item),
        })
    }
}

impl Pool<ImageInfo, Image> for LazyPool {
    fn lease(&mut self, info: ImageInfo) -> Result<Lease<Image>, DriverError> {
        let image_cache = self
            .image_cache
            .entry(ImageKey {
                array_elements: info.array_elements,
                depth: info.depth,
                fmt: info.fmt,
                height: info.height,
                linear_tiling: info.linear_tiling,
                mip_level_count: info.mip_level_count,
                sample_count: info.sample_count,
                ty: info.ty,
                width: info.width,
            })
            .or_default();
        let cache_ref = Arc::clone(image_cache);
        let mut cache = image_cache.lock();

        if cache.is_empty() {
            let item = Image::create(&self.device, info)?;

            return Ok(Lease {
                cache: Some(cache_ref),
                item: Some(item),
            });
        }

        // Look for a compatible image (superset of creation flags and usage flags)
        for idx in 0..cache.len() {
            let item = &cache[idx];
            if item.info.flags.contains(info.flags) && item.info.usage.contains(info.usage) {
                let item = cache.remove(idx).unwrap();

                return Ok(Lease {
                    cache: Some(cache_ref),
                    item: Some(item),
                });
            }
        }

        let item = Image::create(&self.device, info)?;

        Ok(Lease {
            cache: Some(cache_ref),
            item: Some(item),
        })
    }
}

impl Pool<ImageInfoBuilder, Image> for LazyPool {
    fn lease(&mut self, info: ImageInfoBuilder) -> Result<Lease<Image>, DriverError> {
        self.lease(info.build())
    }
}

impl Pool<RenderPassInfo, RenderPass> for LazyPool {
    fn lease(&mut self, info: RenderPassInfo) -> Result<Lease<RenderPass>, DriverError> {
        if let Some(cache) = self.render_pass_cache.get(&info) {
            let item = if let Some(item) = cache.lock().pop_front() {
                item
            } else {
                RenderPass::create(&self.device, info)?
            };

            Ok(Lease {
                cache: Some(cache.clone()),
                item: Some(item),
            })
        } else {
            let cache = Arc::new(Mutex::new(VecDeque::new()));
            self.render_pass_cache.insert(info.clone(), cache.clone());

            let item = RenderPass::create(&self.device, info)?;

            Ok(Lease {
                cache: Some(cache),
                item: Some(item),
            })
        }
    }
}

impl Pool<QueueFamily, CommandBuffer> for LazyPool {
    fn lease(&mut self, queue_family: QueueFamily) -> Result<Lease<CommandBuffer>, DriverError> {
        let cache = self
            .command_buffer_cache
            .entry(queue_family.idx)
            .or_insert_with(|| Arc::new(Mutex::new(VecDeque::new())));
        let cache_ref = Arc::clone(cache);
        let mut cache = cache.lock();

        if cache.is_empty() || !Self::can_lease_command_buffer(cache.front_mut().unwrap()) {
            let item = CommandBuffer::create(&self.device, queue_family)?;

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
