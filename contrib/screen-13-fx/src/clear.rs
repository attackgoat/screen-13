use {
    super::{execute, ExecutionError},
    screen_13::prelude_all::*,
};

/// Clears a color image without any render graph
pub fn clear_color_binding<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    image_binding: impl Into<AnyImageBinding<'a, P>>,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    // Get the raw vk image, info, and most recent access type -> we set a new most recent access
    let mut image_binding = image_binding.into();
    let (image, previous_image_access, _) = image_binding.access_inner(AccessType::TransferWrite);
    let image_info = image.info;
    let image = **image;

    cmd_chain
        .into()
        .push_shared_ref(image_binding.shared_ref())
        .push_execute(move |device, cmd_buf| unsafe {
            CommandBuffer::image_barrier(
                cmd_buf,
                previous_image_access,
                AccessType::TransferWrite,
                image,
                None,
            );
            clear_color_image(
                device,
                **cmd_buf,
                image,
                image_info.mip_level_count,
                image_info.array_elements,
                r,
                g,
                b,
                a,
            );
        })
}

unsafe fn clear_color_image(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    image: vk::Image,
    image_mip_level_count: u32,
    image_array_elements: u32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) {
    device.cmd_clear_color_image(
        cmd_buf,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &vk::ClearColorValue {
            float32: [r, g, b, a],
        },
        &[vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            level_count: image_mip_level_count,
            layer_count: image_array_elements,
            ..Default::default()
        }],
    )
}

/// Clears a color image as part of a render graph but outside of any graphic render pass
pub fn clear_color_node<P>(
    render_graph: &mut RenderGraph<P>,
    image_node: impl Into<AnyImageNode<P>>,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) where
    P: SharedPointerKind + 'static,
{
    let image_node = image_node.into();
    let image_info = render_graph.node_info(image_node);

    render_graph
        .record_pass("clear color")
        .access_node(image_node, AccessType::TransferWrite)
        .execute_pass(move |device, cmd_buf, bindings| unsafe {
            clear_color_image(
                device,
                cmd_buf,
                *bindings[image_node],
                image_info.mip_level_count,
                image_info.array_elements,
                r,
                g,
                b,
                a,
            );
        });
}
