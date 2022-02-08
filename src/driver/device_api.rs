// Just a bunch of helpers to make common code cleaner

// Note: If you want to write something "real" or shippable you probably don't want these functions

// Note: This may be a bad idea overall, as the deref implementation *just happens* to not have any
// of these function names.... but they're so conveinent for people who don't care!

use {
    super::{
        Buffer, BufferInfo, ComputePipeline, ComputePipelineInfo, Device, GraphicPipeline,
        GraphicPipelineInfo, Image, ImageInfo, RayTracePipeline, RayTracePipelineInfo, Shader,
    },
    crate::{graph::ImageBinding, ptr::Shared},
    archery::SharedPointerKind,
};

impl<'a, P> Shared<Device<P>, P>
where
    P: SharedPointerKind,
{
    pub fn new_buffer(&self, info: impl Into<BufferInfo>) -> Shared<Buffer<P>, P> {
        Shared::new(Buffer::create(self, info).unwrap())
    }

    pub fn new_compute_pipeline(
        &self,
        info: impl Into<ComputePipelineInfo>,
    ) -> Shared<ComputePipeline<P>, P> {
        Shared::new(ComputePipeline::create(self, info).unwrap())
    }

    pub fn new_image(&self, info: impl Into<ImageInfo>) -> ImageBinding<P> {
        ImageBinding::new(self.new_image_raw(info))
    }

    pub fn new_image_raw(&self, info: impl Into<ImageInfo>) -> Image<P> {
        Image::create(self, info).unwrap()
    }

    pub fn new_graphic_pipeline<S>(
        &self,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Shared<GraphicPipeline<P>, P>
    where
        S: Into<Shader>,
    {
        Shared::new(GraphicPipeline::create(self, info, shaders).unwrap())
    }

    pub fn new_ray_trace_pipeline<S>(
        &self,
        info: impl Into<RayTracePipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Shared<RayTracePipeline<P>, P>
    where
        S: Into<Shader>,
    {
        Shared::new(RayTracePipeline::create(self, info, shaders).unwrap())
    }
}
