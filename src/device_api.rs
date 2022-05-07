use {
    super::{
        driver::{
            Buffer, BufferInfo, ComputePipeline, ComputePipelineInfo, GraphicPipeline,
            GraphicPipelineInfo, Image, ImageInfo, RayTracePipeline, RayTracePipelineInfo, Shader,
        },
        graph::ImageBinding,
        EventLoop,
    },
    archery::{SharedPointer, SharedPointerKind},
};

impl<P> EventLoop<P>
where
    P: SharedPointerKind + Send,
{
    pub fn new_buffer(&self, info: impl Into<BufferInfo>) -> SharedPointer<Buffer<P>, P> {
        SharedPointer::new(Buffer::create(&self.device, info).unwrap())
    }

    pub fn new_compute_pipeline(
        &self,
        info: impl Into<ComputePipelineInfo>,
    ) -> SharedPointer<ComputePipeline<P>, P> {
        SharedPointer::new(ComputePipeline::create(&self.device, info).unwrap())
    }

    pub fn new_image(&self, info: impl Into<ImageInfo>) -> ImageBinding<P> {
        ImageBinding::new(self.new_image_raw(info))
    }

    pub fn new_image_raw(&self, info: impl Into<ImageInfo>) -> Image<P> {
        Image::create(&self.device, info).unwrap()
    }

    pub fn new_graphic_pipeline<S>(
        &self,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> SharedPointer<GraphicPipeline<P>, P>
    where
        S: Into<Shader>,
    {
        SharedPointer::new(GraphicPipeline::create(&self.device, info, shaders).unwrap())
    }
}
