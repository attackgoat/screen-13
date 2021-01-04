use {
    super::Device,
    gfx_hal::{device::Device as _, pso::GraphicsPipelineDesc, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct GraphicsPipeline {
    device: Device,
    ptr: Option<<_Backend as Backend>::GraphicsPipeline>,
}

impl GraphicsPipeline {
    pub fn new(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        desc: &GraphicsPipelineDesc<'_, _Backend>,
    ) -> Self {
        let graphics_pipeline = unsafe {
                // TODO: Use a pipeline cache?
                let ctor = || device.create_graphics_pipeline(&desc, None).unwrap();

                #[cfg(feature = "debug-names")]
                let mut graphics_pipeline = ctor();

                #[cfg(not(feature = "debug-names"))]
                let graphics_pipeline = ctor();

                #[cfg(feature = "debug-names")]
                device.set_graphics_pipeline_name(&mut graphics_pipeline, name);

                graphics_pipeline
        };

        Self {
            device,
            ptr: Some(graphics_pipeline),
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(graphics_pipeline: &mut Self, name: &str) {
        let device = graphics_pipeline.driver.as_ref().borrow();
        let ptr = graphics_pipeline.ptr.as_mut().unwrap();

        unsafe {
            device.set_graphics_pipeline_name(ptr, name);
        }
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
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for GraphicsPipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_graphics_pipeline(ptr);
        }
    }
}
