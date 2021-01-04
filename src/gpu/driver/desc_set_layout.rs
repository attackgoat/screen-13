use {
    super::Device,
    gfx_hal::{device::Device as _, pso::DescriptorSetLayoutBinding, Backend},
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

pub struct DescriptorSetLayout {
    device: Device,
    ptr: Option<<_Backend as Backend>::DescriptorSetLayout>,
}

impl DescriptorSetLayout {
    pub fn new<I>(#[cfg(feature = "debug-names")] name: &str, device: Device, bindings: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
    {
        // TODO: This driver code does not support the imutable samplers feature.
        // See: `pImmutableSamplers` in https://vulkan.lunarg.com/doc/view/1.2.131.2/windows/vkspec.html#descriptorsets-sets
        let set_layout = unsafe {
            let ctor = || device.create_descriptor_set_layout(bindings, &[]).unwrap();

            #[cfg(feature = "debug-names")]
            let mut set_layout = ctor();

            #[cfg(not(feature = "debug-names"))]
            let set_layout = ctor();

            #[cfg(feature = "debug-names")]
            device.set_descriptor_set_layout_name(&mut set_layout, name);

            set_layout
        };

        Self {
            device,
            ptr: Some(set_layout),
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(set_layout: &mut Self, name: &str) {
        let device = set_layout.driver.as_ref().borrow();
        let ptr = set_layout.ptr.as_mut().unwrap();

        unsafe {
            device.set_descriptor_set_layout_name(ptr, name);
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
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_descriptor_set_layout(ptr);
        }
    }
}
