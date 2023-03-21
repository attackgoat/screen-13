use {
    super::{device::Device, DriverError},
    ash::vk,
    log::warn,
    std::{ops::Deref, sync::Arc, thread::panicking},
};

#[derive(Debug)]
pub struct DescriptorSetLayout {
    device: Arc<Device>,
    descriptor_set_layout: vk::DescriptorSetLayout,
}

impl DescriptorSetLayout {
    pub fn create(
        device: &Arc<Device>,
        info: &vk::DescriptorSetLayoutCreateInfo,
    ) -> Result<Self, DriverError> {
        let device = Arc::clone(device);
        let descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })
        }?;

        Ok(Self {
            device,
            descriptor_set_layout,
        })
    }
}

impl Deref for DescriptorSetLayout {
    type Target = vk::DescriptorSetLayout;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_set_layout
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}
