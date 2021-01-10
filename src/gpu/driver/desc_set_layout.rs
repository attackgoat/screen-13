use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, pso::DescriptorSetLayoutBinding, Backend},
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

pub struct DescriptorSetLayout(Option<<_Backend as Backend>::DescriptorSetLayout>);

impl DescriptorSetLayout {
    pub unsafe fn new<I>(#[cfg(feature = "debug-names")] name: &str, bindings: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
    {
        // TODO: This driver code does not support the imutable samplers feature.
        // See: `pImmutableSamplers` at
        // https://vulkan.lunarg.com/doc/view/1.2.131.2/windows/vkspec.html#descriptorsets-sets
        let ctor = || {
            device()
                .create_descriptor_set_layout(bindings, &[])
                .unwrap()
        };

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_descriptor_set_layout_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(layout: &mut Self, name: &str) {
        let ptr = layout.0.as_mut().unwrap();
        device().set_descriptor_set_layout_name(ptr, name);
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for DescriptorSetLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_descriptor_set_layout(ptr);
        }
    }
}
