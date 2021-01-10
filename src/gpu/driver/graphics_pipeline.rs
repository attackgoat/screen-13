use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, pso::GraphicsPipelineDesc, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct GraphicsPipeline(Option<<_Backend as Backend>::GraphicsPipeline>);

impl GraphicsPipeline {
    pub unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        desc: &GraphicsPipelineDesc<'_, _Backend>,
    ) -> Self {
        // TODO: Use a pipeline cache?
        let ctor = || device().create_graphics_pipeline(&desc, None).unwrap();

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_graphics_pipeline_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(pipeline: &mut Self, name: &str) {
        let ptr = pipeline.0.as_mut().unwrap();
        device().set_graphics_pipeline_name(ptr, name);
    }
}

impl AsMut<<_Backend as Backend>::GraphicsPipeline> for GraphicsPipeline {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::GraphicsPipeline {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::GraphicsPipeline> for GraphicsPipeline {
    fn as_ref(&self) -> &<_Backend as Backend>::GraphicsPipeline {
        &*self
    }
}

impl Deref for GraphicsPipeline {
    type Target = <_Backend as Backend>::GraphicsPipeline;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for GraphicsPipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_graphics_pipeline(ptr);
        }
    }
}
