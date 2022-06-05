use {
    super::{DescriptorSetLayout, Device, DriverError},
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    std::{ops::Deref, sync::Arc, thread::panicking},
};

#[derive(Debug)]
pub struct DescriptorPool {
    pub info: DescriptorPoolInfo,
    descriptor_pool: vk::DescriptorPool,
    pub device: Arc<Device>,
}

impl DescriptorPool {
    pub fn create(
        device: &Arc<Device>,
        info: impl Into<DescriptorPoolInfo>,
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let info = info.into();
        let descriptor_pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                    .max_sets(info.max_sets)
                    .pool_sizes(
                        &info
                            .pool_sizes
                            .iter()
                            .map(|pool_size| vk::DescriptorPoolSize {
                                ty: pool_size.ty,
                                descriptor_count: pool_size.descriptor_count,
                            })
                            .collect::<Box<[_]>>(),
                    ),
                None,
            )
        }
        .map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;

        Ok(Self {
            descriptor_pool,
            device,
            info,
        })
    }

    pub fn allocate_descriptor_set(
        this: &Self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, DriverError> {
        Ok(Self::allocate_descriptor_sets(this, layout, 1)?
            .next()
            .unwrap())
    }

    pub fn allocate_descriptor_sets<'a>(
        this: &'a Self,
        layout: &DescriptorSetLayout,
        count: u32,
    ) -> Result<impl Iterator<Item = DescriptorSet> + 'a, DriverError> {
        use std::slice::from_ref;

        let mut create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(this.descriptor_pool)
            .set_layouts(from_ref(layout));
        create_info.descriptor_set_count = count;

        trace!("allocate_descriptor_sets");

        Ok(unsafe {
            this.device
                .allocate_descriptor_sets(&create_info)
                .map_err(|err| {
                    use {vk::Result as vk, DriverError::*};

                    warn!("{err}");

                    match err {
                        e if e == vk::ERROR_FRAGMENTED_POOL => InvalidData,
                        e if e == vk::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
                        e if e == vk::ERROR_OUT_OF_HOST_MEMORY => OutOfMemory,
                        e if e == vk::ERROR_OUT_OF_POOL_MEMORY => OutOfMemory,
                        _ => Unsupported,
                    }
                })?
                .into_iter()
                .map(move |descriptor_set| DescriptorSet {
                    descriptor_pool: this.descriptor_pool,
                    descriptor_set,
                    device: Arc::clone(&this.device),
                })
        })
    }
}

impl Deref for DescriptorPool {
    type Target = vk::DescriptorPool;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_pool
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}

#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned", derive(Debug))]
pub struct DescriptorPoolInfo {
    pub max_sets: u32,
    pub pool_sizes: Vec<DescriptorPoolSize>,
}

impl DescriptorPoolInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(max_sets: u32) -> DescriptorPoolInfoBuilder {
        DescriptorPoolInfoBuilder::default().max_sets(max_sets)
    }
}

impl From<DescriptorPoolInfoBuilder> for DescriptorPoolInfo {
    fn from(info: DescriptorPoolInfoBuilder) -> Self {
        info.build().unwrap()
    }
}

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(pattern = "owned")]
pub struct DescriptorPoolSize {
    pub ty: vk::DescriptorType,
    pub descriptor_count: u32,
}

#[derive(Debug)]
pub struct DescriptorSet {
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    device: Arc<Device>,
}

impl Deref for DescriptorSet {
    type Target = vk::DescriptorSet;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_set
    }
}

impl Drop for DescriptorSet {
    fn drop(&mut self) {
        use std::slice::from_ref;

        if panicking() {
            return;
        }

        unsafe {
            self.device
                .free_descriptor_sets(self.descriptor_pool, from_ref(&self.descriptor_set))
                .unwrap_or_else(|_| warn!("Unable to free descriptor set"))
        }
    }
}
