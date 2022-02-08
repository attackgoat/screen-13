use {
    super::{Device, DriverError},
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::vk,
    std::{ops::Deref, thread::panicking},
};

#[derive(Debug)]
pub struct DescriptorSetLayout<P>
where
    P: SharedPointerKind,
{
    device: Shared<Device<P>, P>,
    descriptor_set_layout: vk::DescriptorSetLayout,
}

impl<P> DescriptorSetLayout<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &Shared<Device<P>, P>,
        info: &vk::DescriptorSetLayoutCreateInfo,
    ) -> Result<Self, DriverError>
    where
        P: SharedPointerKind,
    {
        let device = Shared::clone(device);
        let descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(info, None)
                .map_err(|_| DriverError::Unsupported)
        }?;

        Ok(Self {
            device,
            descriptor_set_layout,
        })
    }
}

impl<P> Deref for DescriptorSetLayout<P>
where
    P: SharedPointerKind,
{
    type Target = vk::DescriptorSetLayout;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_set_layout
    }
}

impl<P> Drop for DescriptorSetLayout<P>
where
    P: SharedPointerKind,
{
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
