//! TODO

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
    std::{collections::HashMap, sync::Arc},
};

/// A space-efficient resource allocator.
#[derive(Debug)]
pub struct FifoPool {
    accel_struct_cache: Cache<AccelerationStructure>,
    buffer_cache: Cache<Buffer>,
    command_buffer_cache: HashMap<u32, Cache<CommandBuffer>>,
    descriptor_pool_cache: Cache<DescriptorPool>,
    device: Arc<Device>,
    image_cache: Cache<Image>,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass>>,
}

impl FifoPool {
    /// Constructs a new `FifoPool`.
    pub fn new(device: &Arc<Device>) -> Self {
        Self::with_capacity(device, PoolInfo::default())
    }

    /// Constructs a new `FifoPool` with the given capacity information.
    pub fn with_capacity(device: &Arc<Device>, info: impl Into<PoolInfo>) -> Self {
        let info: PoolInfo = info.into();
        let device = Arc::clone(device);

        Self {
            accel_struct_cache: PoolInfo::explicit_cache(info.accel_struct_capacity),
            buffer_cache: PoolInfo::explicit_cache(info.buffer_capacity),
            command_buffer_cache: Default::default(),
            descriptor_pool_cache: PoolInfo::default_cache(),
            device,
            image_cache: PoolInfo::explicit_cache(info.image_capacity),
            render_pass_cache: Default::default(),
        }
    }
}

impl Pool<AccelerationStructureInfo, AccelerationStructure> for FifoPool {
    #[profiling::function]
    fn lease(
        &mut self,
        info: AccelerationStructureInfo,
    ) -> Result<Lease<AccelerationStructure>, DriverError> {
        let cache_ref = Arc::downgrade(&self.accel_struct_cache);
        let mut cache = self.accel_struct_cache.lock();

        {
            profiling::scope!("Check cache");

            // Look for a compatible acceleration structure (big enough and same type)
            for idx in 0..cache.len() {
                let item = &cache[idx];
                if item.info.size >= info.size && item.info.ty == info.ty {
                    let item = cache.remove(idx).unwrap();

                    return Ok(Lease::new(cache_ref, item));
                }
            }
        }

        let item = AccelerationStructure::create(&self.device, info)?;

        Ok(Lease::new(cache_ref, item))
    }
}

impl Pool<BufferInfo, Buffer> for FifoPool {
    #[profiling::function]
    fn lease(&mut self, info: BufferInfo) -> Result<Lease<Buffer>, DriverError> {
        let cache_ref = Arc::downgrade(&self.buffer_cache);
        let mut cache = self.buffer_cache.lock();

        {
            profiling::scope!("Check cache");

            // Look for a compatible buffer (compatible alignment, same mapping mode, big enough and
            // superset of usage flags)
            for idx in 0..cache.len() {
                let item = &cache[idx];
                if item.info.alignment >= info.alignment
                    && item.info.can_map == info.can_map
                    && item.info.size >= info.size
                    && item.info.usage.contains(info.usage)
                {
                    let item = cache.remove(idx).unwrap();

                    return Ok(Lease::new(cache_ref, item));
                }
            }
        }

        let item = Buffer::create(&self.device, info)?;

        Ok(Lease::new(cache_ref, item))
    }
}

impl Pool<CommandBufferInfo, CommandBuffer> for FifoPool {
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
            .unwrap_or_else(|| CommandBuffer::create(&self.device, info))?;

        // Drop anything we were holding from the last submission
        CommandBuffer::drop_fenced(&mut item);

        Ok(Lease::new(Arc::downgrade(cache), item))
    }
}

impl Pool<DescriptorPoolInfo, DescriptorPool> for FifoPool {
    #[profiling::function]
    fn lease(&mut self, info: DescriptorPoolInfo) -> Result<Lease<DescriptorPool>, DriverError> {
        let cache_ref = Arc::downgrade(&self.descriptor_pool_cache);
        let mut cache = self.descriptor_pool_cache.lock();

        {
            profiling::scope!("Check cache");

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

                    return Ok(Lease::new(cache_ref, item));
                }
            }
        }

        let item = DescriptorPool::create(&self.device, info)?;

        Ok(Lease::new(cache_ref, item))
    }
}

impl Pool<ImageInfo, Image> for FifoPool {
    #[profiling::function]
    fn lease(&mut self, info: ImageInfo) -> Result<Lease<Image>, DriverError> {
        let cache_ref = Arc::downgrade(&self.image_cache);
        let mut cache = self.image_cache.lock();

        {
            profiling::scope!("Check cache");

            // Look for a compatible image (same properties, superset of creation flags and usage
            // flags)
            for idx in 0..cache.len() {
                let item = &cache[idx];
                if item.info.array_elements == info.array_elements
                    && item.info.depth == info.depth
                    && item.info.fmt == info.fmt
                    && item.info.height == info.height
                    && item.info.linear_tiling == info.linear_tiling
                    && item.info.mip_level_count == info.mip_level_count
                    && item.info.sample_count == info.sample_count
                    && item.info.ty == info.ty
                    && item.info.width == info.width
                    && item.info.flags.contains(info.flags)
                    && item.info.usage.contains(info.usage)
                {
                    let item = cache.remove(idx).unwrap();

                    return Ok(Lease::new(cache_ref, item));
                }
            }
        }

        let item = Image::create(&self.device, info)?;

        Ok(Lease::new(cache_ref, item))
    }
}

impl Pool<RenderPassInfo, RenderPass> for FifoPool {
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
        let item = cache
            .lock()
            .pop_front()
            .map(Ok)
            .unwrap_or_else(|| RenderPass::create(&self.device, info))?;

        Ok(Lease::new(Arc::downgrade(cache), item))
    }
}
