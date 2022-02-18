use screen_13::prelude_all::*;

pub struct ComputePresenter<P>([Shared<ComputePipeline<P>, P>; 2])
where
    P: SharedPointerKind;

impl<P> ComputePresenter<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        let pipeline1 = Shared::new(ComputePipeline::create(
            device,
            ComputePipelineInfo::new(crate::res::shader::COMPUTE_PRESENT1_COMP),
        )?);
        let pipeline2 = Shared::new(ComputePipeline::create(
            device,
            ComputePipelineInfo::new(crate::res::shader::COMPUTE_PRESENT2_COMP),
        )?);

        Ok(Self([pipeline1, pipeline2]))
    }

    pub fn present_image(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        swapchain: SwapchainImageNode<P>,
    ) where
        P: 'static,
    {
        let image = image.into();
        // let image_info = graph.node_info(image);
        let swapchain_info = graph.node_info(swapchain);

        // TODO: Notice non-sRGB images and run a different pipeline

        graph
            .record_pass("present (from compute)")
            .bind_pipeline(&self.0[0])
            .read_descriptor(0, image)
            .write_descriptor(1, swapchain)
            .dispatch(swapchain_info.extent.x, swapchain_info.extent.y, 1);
    }

    pub fn present_images(
        &self,
        graph: &mut RenderGraph<P>,
        top_image: impl Into<AnyImageNode<P>>,
        bottom_image: impl Into<AnyImageNode<P>>,
        swapchain: SwapchainImageNode<P>,
    ) where
        P: 'static,
    {
        let top_image = top_image.into();
        let bottom_image = bottom_image.into();
        // let top_image_info = graph.node_info(top_image);
        // let bottom_image_info = graph.node_info(bottom_image);
        let swapchain_info = graph.node_info(swapchain);

        // TODO: Notice non-sRGB images and run a different pipeline

        graph
            .record_pass("present (from compute)")
            .bind_pipeline(&self.0[1])
            .read_descriptor((0, [0]), top_image)
            .read_descriptor((0, [1]), bottom_image)
            .write_descriptor(1, swapchain)
            .dispatch(swapchain_info.extent.x, swapchain_info.extent.y, 1);
    }
}

pub struct GraphicPresenter<P>(Shared<GraphicPipeline<P>, P>)
where
    P: SharedPointerKind;

impl<P> GraphicPresenter<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self(Shared::new(GraphicPipeline::create(
            device,
            GraphicPipelineInfo::new(),
            [
                Shader::new_vertex(crate::res::shader::GRAPHIC_PRESENT_VERT),
                Shader::new_fragment(crate::res::shader::GRAPHIC_PRESENT_FRAG),
            ],
        )?)))
    }

    pub fn present_image(
        &self,
        graph: &mut RenderGraph<P>,
        image: impl Into<AnyImageNode<P>>,
        swapchain: SwapchainImageNode<P>,
    ) where
        P: 'static,
    {
        let image = image.into();
        let image_info = graph.node_info(image);
        let swapchain_info = graph.node_info(swapchain);

        let image_extent = image_info.extent.xy().as_vec2();
        let swapchain_extent = swapchain_info.extent.xy().as_vec2();
        let scale = (swapchain_extent.x / image_extent.x).max(swapchain_extent.y / image_extent.y);
        let transform = Mat4::from_scale(vec3(
            scale * image_extent.x / swapchain_extent.x,
            scale * image_extent.y / swapchain_extent.y,
            1.0,
        ));

        graph
            .record_pass("present (from graphic)")
            .bind_pipeline(&self.0)
            .read_descriptor(0, image)
            .store_color(0, swapchain)
            .push_constants(transform)
            .draw(|device, cmd_buf, _bindings| unsafe {
                // Draw a quad with implicit vertices (no buffer)
                // TODO: Reduce vertex count
                // https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/
                device.cmd_draw(cmd_buf, 6, 1, 0, 0);
            });
    }
}
