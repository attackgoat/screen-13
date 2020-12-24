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

pub struct ComputePipeline {
    driver: Driver,
    layout: PipelineLayout,
    ptr: Option<<_Backend as Backend>::ComputePipeline>,
}

impl ComputePipeline {
    pub unsafe fn new<IS, IR>(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        entry_point: EntryPoint<'_, _Backend>,
        set_layouts: IS,
        push_consts: IR,
    ) -> Self
    where
        IS: IntoIterator,
        IS::Item: Borrow<<_Backend as Backend>::DescriptorSetLayout>,
        IS::IntoIter: ExactSizeIterator,
        IR: IntoIterator,
        IR::Item: Borrow<(ShaderStageFlags, Range<u32>)>,
        IR::IntoIter: ExactSizeIterator,
    {
        let layout = PipelineLayout::new(
            #[cfg(debug_assertions)]
            &format!("{} Pipeline layout", name),
            Driver::clone(&driver),
            set_layouts,
            push_consts,
        );
        let desc = ComputePipelineDesc::new(entry_point, &*layout);
        let compute_pipeline = driver
            .as_ref()
            .borrow()
            .create_compute_pipeline(&desc, None)
            .unwrap();

        Self {
            ptr: Some(compute_pipeline),
            driver,
            layout,
        }
    }

    pub fn layout(pipeline: &Self) -> &<_Backend as Backend>::PipelineLayout {
        &pipeline.layout
    }
}

impl AsMut<<_Backend as Backend>::ComputePipeline> for ComputePipeline {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::ComputePipeline {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::ComputePipeline> for ComputePipeline {
    fn as_ref(&self) -> &<_Backend as Backend>::ComputePipeline {
        &*self
    }
}

impl Deref for ComputePipeline {
    type Target = <_Backend as Backend>::ComputePipeline;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for ComputePipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_compute_pipeline(ptr);
        }
    }
}
