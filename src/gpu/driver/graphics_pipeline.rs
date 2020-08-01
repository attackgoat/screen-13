use {
    super::Driver,
    gfx_hal::{device::Device, pso::GraphicsPipelineDesc, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct GraphicsPipeline {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::GraphicsPipeline>,
}

impl GraphicsPipeline {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        desc: &GraphicsPipelineDesc<'_, _Backend>,
    ) -> Self {
        #[cfg(debug_assertions)]
        debug!("Creating graphics pipeline '{}'", name);

        let graphics_pipeline = {
            let device = driver.borrow();

            unsafe { device.create_graphics_pipeline(&desc, None) }.unwrap()
        }; // TODO: Use a pipeline cache?

        Self {
            driver,
            ptr: Some(graphics_pipeline),
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
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_graphics_pipeline(ptr);
        }
    }
}
