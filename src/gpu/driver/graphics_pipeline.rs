use {
    super::Driver,
    gfx_hal::{device::Device, pso::GraphicsPipelineDesc, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct GraphicsPipeline {
    driver: Driver,
    graphics_pipeline: Option<<_Backend as Backend>::GraphicsPipeline>,
}

impl GraphicsPipeline {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        desc: &GraphicsPipelineDesc<'_, _Backend>,
    ) -> Self {
        #[cfg(debug_assertions)]
        debug!("Creating graphics pipeline '{}'", name);

        let graphics_pipeline = unsafe {
            driver
                .as_ref()
                .borrow()
                .create_graphics_pipeline(&desc, None)
        }
        .unwrap(); // TODO: Use a pipeline cache?

        Self {
            driver,
            graphics_pipeline: Some(graphics_pipeline),
        }
    }
}

impl AsRef<<_Backend as Backend>::GraphicsPipeline> for GraphicsPipeline {
    fn as_ref(&self) -> &<_Backend as Backend>::GraphicsPipeline {
        self.graphics_pipeline.as_ref().unwrap()
    }
}

impl Deref for GraphicsPipeline {
    type Target = <_Backend as Backend>::GraphicsPipeline;

    fn deref(&self) -> &Self::Target {
        self.graphics_pipeline.as_ref().unwrap()
    }
}

impl DerefMut for GraphicsPipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.graphics_pipeline.as_mut().unwrap()
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_graphics_pipeline(self.graphics_pipeline.take().unwrap());
        }
    }
}
