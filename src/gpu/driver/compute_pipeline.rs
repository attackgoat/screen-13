use {
    super::Device,
    gfx_hal::{
        device::Device as _,
        pso::{ComputePipelineDesc, EntryPoint},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct ComputePipeline {
    device: Device,
    ptr: Option<<_Backend as Backend>::ComputePipeline>,
}

impl ComputePipeline {
    pub unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        layout: &<_Backend as Backend>::PipelineLayout,
        entry_point: EntryPoint<'_, _Backend>,
    ) -> Self {
        let desc = ComputePipelineDesc::new(entry_point, layout);
        let compute_pipeline = {
            let ctor = || device.create_compute_pipeline(&desc, None).unwrap();

            #[cfg(feature = "debug-names")]
            let mut compute_pipeline = ctor();

            #[cfg(not(feature = "debug-names"))]
            let compute_pipeline = ctor();

            #[cfg(feature = "debug-names")]
            device.set_compute_pipeline_name(&mut compute_pipeline, name);

            compute_pipeline
        };

        Self {
            ptr: Some(compute_pipeline),
            device,
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(feature = "debug-names")]
    pub fn set_name(compute_pipeline: &mut Self, name: &str) {
        let ptr = compute_pipeline.ptr.as_mut().unwrap();

        unsafe {
            compute_pipeline.device.set_compute_pipeline_name(ptr, name);
        }
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
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_compute_pipeline(ptr);
        }
    }
}
