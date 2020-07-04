use {
    super::Driver,
    gfx_hal::{device::Device, pso::DescriptorSetLayoutBinding, Backend},
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

#[derive(Debug)]
pub struct DescriptorSetLayout {
    set_layout: Option<<_Backend as Backend>::DescriptorSetLayout>,
    driver: Driver,
}

impl DescriptorSetLayout {
    pub fn new<I>(#[cfg(debug_assertions)] name: &str, driver: Driver, bindings: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
    {
        // TODO: This driver code does not support the imutable samplers feature.
        // See: `pImmutableSamplers` in https://vulkan.lunarg.com/doc/view/1.2.131.2/windows/vkspec.html#descriptorsets-sets
        let set_layout = {
            let device = driver.as_ref().borrow();
            let ctor = || unsafe { device.create_descriptor_set_layout(bindings, &[]) }.unwrap();

            #[cfg(debug_assertions)]
            let mut set_layout = ctor();
            #[cfg(not(debug_assertions))]
            let set_layout = ctor();

            #[cfg(debug_assertions)]
            unsafe {
                device.set_descriptor_set_layout_name(&mut set_layout, name);
            }

            set_layout
        };

        Self {
            set_layout: Some(set_layout),
            driver,
        }
    }
}

impl AsRef<<_Backend as Backend>::DescriptorSetLayout> for DescriptorSetLayout {
    fn as_ref(&self) -> &<_Backend as Backend>::DescriptorSetLayout {
        self.set_layout.as_ref().unwrap()
    }
}

impl Deref for DescriptorSetLayout {
    type Target = <_Backend as Backend>::DescriptorSetLayout;

    fn deref(&self) -> &Self::Target {
        self.set_layout.as_ref().unwrap()
    }
}

impl DerefMut for DescriptorSetLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_layout.as_mut().unwrap()
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_descriptor_set_layout(self.set_layout.take().unwrap());
        }
    }
}
