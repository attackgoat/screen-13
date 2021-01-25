use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, pso::ShaderStageFlags, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut, Range},
};

pub struct PipelineLayout(Option<<_Backend as Backend>::PipelineLayout>);

impl PipelineLayout {
    pub unsafe fn new<'a, Is, Ic>(
        #[cfg(feature = "debug-names")] name: &str,
        set_layouts: Is,
        push_consts: Ic,
    ) -> Self
    where
        Is: IntoIterator<Item = &'a <_Backend as Backend>::DescriptorSetLayout>,
        Is::IntoIter: ExactSizeIterator,
        Ic: IntoIterator<Item = (ShaderStageFlags, Range<u32>)>,
        Ic::IntoIter: ExactSizeIterator,
    {
        let ctor = || {
            device()
                .create_pipeline_layout(set_layouts, push_consts)
                .unwrap()
        };

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_pipeline_layout_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(layout: &mut Self, name: &str) {
        let ptr = layout.0.as_mut().unwrap();
        device().set_pipeline_layout_name(ptr, name);
    }
}

impl AsMut<<_Backend as Backend>::PipelineLayout> for PipelineLayout {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::PipelineLayout {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::PipelineLayout> for PipelineLayout {
    fn as_ref(&self) -> &<_Backend as Backend>::PipelineLayout {
        &*self
    }
}

impl Deref for PipelineLayout {
    type Target = <_Backend as Backend>::PipelineLayout;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for PipelineLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_pipeline_layout(ptr);
        }
    }
}
