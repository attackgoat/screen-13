use {
    super::{
        driver::{
            Buffer, BufferInfo, ComputePipeline, ComputePipelineInfo, GraphicPipeline,
            GraphicPipelineInfo, Image, ImageInfo, Shader,
        },
        event_loop::EventLoop,
        graph::ImageBinding,
    },
    std::sync::Arc,
};

impl EventLoop {
    pub fn new_buffer(&self, info: impl Into<BufferInfo>) -> Arc<Buffer> {
        Arc::new(Buffer::create(&self.device, info).unwrap())
    }

    pub fn new_compute_pipeline(
        &self,
        info: impl Into<ComputePipelineInfo>,
    ) -> Arc<ComputePipeline> {
        Arc::new(ComputePipeline::create(&self.device, info).unwrap())
    }

    pub fn new_image(&self, info: impl Into<ImageInfo>) -> ImageBinding {
        ImageBinding::new(self.new_image_raw(info))
    }

    pub fn new_image_raw(&self, info: impl Into<ImageInfo>) -> Image {
        Image::create(&self.device, info).unwrap()
    }

    pub fn new_graphic_pipeline<S>(
        &self,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Arc<GraphicPipeline>
    where
        S: Into<Shader>,
    {
        Arc::new(GraphicPipeline::create(&self.device, info, shaders).unwrap())
    }
}
