use {
    super::{Driver, PipelineLayout},
    gfx_hal::{
        device::Device,
        pso::{ComputePipelineDesc, EntryPoint, ShaderStageFlags},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut, Range},
    },
};

#[derive(Debug)]
pub struct ComputePipeline {
    compute_pipeline: Option<<_Backend as Backend>::ComputePipeline>,
    driver: Driver,
    layout: PipelineLayout,
}

impl ComputePipeline {
    pub unsafe fn new<IS, IR>(
        driver: Driver,
        entry_point: EntryPoint<'_, _Backend>,
        set_layouts: IS,
        push_constants: IR,
    ) -> Self
    where
        IS: IntoIterator,
        IS::Item: Borrow<<_Backend as Backend>::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(ShaderStageFlags, Range<u32>)>,
    {
        let layout = PipelineLayout::new(Driver::clone(&driver), set_layouts, push_constants);
        let desc = ComputePipelineDesc::new(entry_point, layout.as_ref());
        let compute_pipeline = driver
            .as_ref()
            .borrow()
            .create_compute_pipeline(&desc, None)
            .unwrap();

        Self {
            compute_pipeline: Some(compute_pipeline),
            driver,
            layout,
        }
    }

    pub fn layout(&self) -> &<_Backend as Backend>::PipelineLayout {
        &self.layout
    }
}

impl AsRef<<_Backend as Backend>::ComputePipeline> for ComputePipeline {
    fn as_ref(&self) -> &<_Backend as Backend>::ComputePipeline {
        self.compute_pipeline.as_ref().unwrap()
    }
}

impl Deref for ComputePipeline {
    type Target = <_Backend as Backend>::ComputePipeline;

    fn deref(&self) -> &Self::Target {
        self.compute_pipeline.as_ref().unwrap()
    }
}

impl DerefMut for ComputePipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.compute_pipeline.as_mut().unwrap()
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_compute_pipeline(self.compute_pipeline.take().unwrap());
        }
    }
}
