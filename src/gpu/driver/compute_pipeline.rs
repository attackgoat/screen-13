use {
    crate::gpu::device,
    gfx_hal::{
        device::Device as _,
        pso::{ComputePipelineDesc, EntryPoint},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct ComputePipeline(Option<<_Backend as Backend>::ComputePipeline>);

impl ComputePipeline {
    pub unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        layout: &<_Backend as Backend>::PipelineLayout,
        entry_point: EntryPoint<'_, _Backend>,
    ) -> Self {
        let desc = ComputePipelineDesc::new(entry_point, layout);
        let ctor = || device().create_compute_pipeline(&desc, None).unwrap();

        #[cfg(feature = "debug-names")]
        let mut ptr = ctor();

        #[cfg(not(feature = "debug-names"))]
        let ptr = ctor();

        #[cfg(feature = "debug-names")]
        device().set_compute_pipeline_name(&mut ptr, name);

        Self(Some(ptr))
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as
    /// [RenderDoc](https://renderdoc.org/).
    #[cfg(feature = "debug-names")]
    pub unsafe fn set_name(pipeline: &mut Self, name: &str) {
        let ptr = pipeline.0.as_mut().unwrap();
        device().set_compute_pipeline_name(ptr, name);
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for ComputePipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_compute_pipeline(ptr);
        }
    }
}
