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
    driver: Driver,
    ptr: Option<<_Backend as Backend>::DescriptorSetLayout>,
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
            driver,
            ptr: Some(set_layout),
        }
    }
}

impl AsMut<<_Backend as Backend>::DescriptorSetLayout> for DescriptorSetLayout {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::DescriptorSetLayout {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::DescriptorSetLayout> for DescriptorSetLayout {
    fn as_ref(&self) -> &<_Backend as Backend>::DescriptorSetLayout {
        &*self
    }
}

impl Deref for DescriptorSetLayout {
    type Target = <_Backend as Backend>::DescriptorSetLayout;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for DescriptorSetLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_descriptor_set_layout(ptr);
        }
    }
}
