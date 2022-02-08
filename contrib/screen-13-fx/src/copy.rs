use {
    super::{execute, ExecutionError},
    screen_13::prelude_all::*,
    std::ops::Range,
};

unsafe fn copy_buffer(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    src_buf: vk::Buffer,
    dst_buf: vk::Buffer,
    regions: &[vk::BufferCopy],
) {
    device.cmd_copy_buffer(cmd_buf, src_buf, dst_buf, regions);
}

pub fn copy_buffer_binding<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyBufferBinding<'a, P>>,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let src_binding = src_binding.into();
    let src_info = src_binding.as_ref().info;

    copy_buffer_binding_region(
        cmd_chain,
        src_binding,
        dst_binding,
        &vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: src_info.size,
        },
    )
}

pub fn copy_buffer_binding_region<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyBufferBinding<'a, P>>,
    region: &vk::BufferCopy,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    use std::slice::from_ref;

    copy_buffer_binding_regions(cmd_chain, src_binding, dst_binding, from_ref(region))
}

pub fn copy_buffer_binding_regions<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyBufferBinding<'a, P>>,
    regions: &[vk::BufferCopy],
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let mut src_binding = src_binding.into();
    let mut dst_binding = dst_binding.into();

    // Get the driver buffers and most recent access types
    let (src, previous_src_access, _) = src_binding.access_inner(AccessType::TransferRead);
    let (dst, previous_dst_access, _) = dst_binding.access_inner(AccessType::TransferWrite);

    assert!(src.info.usage.contains(vk::BufferUsageFlags::TRANSFER_SRC));
    assert!(dst.info.usage.contains(vk::BufferUsageFlags::TRANSFER_DST));

    // Get the raw vk handles
    let src = **src;
    let dst = **dst;



    // TODO: Maybe calculate subresource usage based on regions, it could add efficiency?
    let regions = regions.to_vec();

    cmd_chain
        .into()
        .push_shared_ref(src_binding.shared_ref())
        .push_shared_ref(dst_binding.shared_ref())
        .push_execute(move |device, cmd_buf| unsafe {
            CommandBuffer::buffer_barrier(
                cmd_buf,
                previous_src_access,
                AccessType::TransferRead,
                src,
                None,
            );
            CommandBuffer::buffer_barrier(
                cmd_buf,
                previous_dst_access,
                AccessType::TransferWrite,
                dst,
                None,
            );
            copy_buffer(device, **cmd_buf, src, dst, &regions);
        })
}

pub fn copy_buffer_binding_to_image<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let dst_binding = dst_binding.into();
    let dst_info = dst_binding.as_ref().info;

    copy_buffer_binding_to_image_region(
        cmd_chain,
        src_binding,
        dst_binding,
        &vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: dst_info.extent.x,
            buffer_image_height: dst_info.extent.y,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: format_aspect_mask(dst_info.fmt),
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: Default::default(),
            image_extent: vk::Extent3D {
                depth: dst_info.extent.z,
                height: dst_info.extent.y,
                width: dst_info.extent.x,
            },
        },
    )
}

pub fn copy_buffer_binding_to_image_region<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
    region: &vk::BufferImageCopy,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    use std::slice::from_ref;

    copy_buffer_binding_to_image_regions(cmd_chain, src_binding, dst_binding, from_ref(region))
}

pub fn copy_buffer_binding_to_image_regions<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyBufferBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
    regions: &[vk::BufferImageCopy],
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let mut src_binding = src_binding.into();
    let mut dst_binding = dst_binding.into();

    // Get the driver buffer/image and most recent access types
    let (src, previous_src_access, _) = src_binding.access_inner(AccessType::TransferRead);
    let (dst, previous_dst_access, _) = dst_binding.access_inner(AccessType::TransferWrite);

    assert!(src.info.usage.contains(vk::BufferUsageFlags::TRANSFER_SRC));
    assert!(dst.info.usage.contains(vk::ImageUsageFlags::TRANSFER_DST));

    // Get the raw vk handles
    let src = **src;
    let dst = **dst;

    // TODO: Maybe calculate subresource usage based on regions, it could add efficiency?
    let regions = regions.to_vec();

    cmd_chain
        .into()
        .push_shared_ref(src_binding.shared_ref())
        .push_shared_ref(dst_binding.shared_ref())
        .push_execute(move |device, cmd_buf| unsafe {
            CommandBuffer::buffer_barrier(
                cmd_buf,
                previous_src_access,
                AccessType::TransferRead,
                src,
                None,
            );
            CommandBuffer::image_barrier(
                cmd_buf,
                previous_dst_access,
                AccessType::TransferWrite,
                dst,
                None,
            );
            copy_buffer_to_image(device, **cmd_buf, src, dst, &regions);
        })
}

unsafe fn copy_buffer_to_image(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    src_buf: vk::Buffer,
    dst_image: vk::Image,
    regions: &[vk::BufferImageCopy],
) {
    device.cmd_copy_buffer_to_image(
        cmd_buf,
        src_buf,
        dst_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        regions,
    );
}

pub fn copy_image_binding<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyImageBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let src_binding = src_binding.into();
    let src_info = src_binding.as_ref().info;

    let dst_binding = dst_binding.into();
    let dst_info = dst_binding.as_ref().info;

    copy_image_binding_region(
        cmd_chain,
        src_binding,
        dst_binding,
        &vk::ImageCopy {
            src_subresource: vk::ImageSubresourceLayers {
                aspect_mask: format_aspect_mask(src_info.fmt),
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            src_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            dst_subresource: vk::ImageSubresourceLayers {
                aspect_mask: format_aspect_mask(dst_info.fmt),
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            dst_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            extent: vk::Extent3D {
                depth: src_info.extent.z.min(dst_info.extent.z),
                height: src_info.extent.y.min(dst_info.extent.y),
                width: src_info.extent.x.min(dst_info.extent.x),
            },
        },
    )
}

pub fn copy_image_binding_region<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyImageBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
    region: &vk::ImageCopy,
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    use std::slice::from_ref;

    copy_image_binding_regions(cmd_chain, src_binding, dst_binding, from_ref(region))
}

pub fn copy_image_binding_regions<'a, Ch, Cb, P>(
    cmd_chain: Ch,
    src_binding: impl Into<AnyImageBinding<'a, P>>,
    dst_binding: impl Into<AnyImageBinding<'a, P>>,
    regions: &[vk::ImageCopy],
) -> CommandChain<Cb, P>
where
    Ch: Into<CommandChain<Cb, P>>,
    Cb: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind + 'static,
{
    let mut src_binding = src_binding.into();
    let mut dst_binding = dst_binding.into();

    // Get the driver images and most recent access types
    let (src, previous_src_access, _) = src_binding.access_inner(AccessType::TransferRead);
    let (dst, previous_dst_access, _) = dst_binding.access_inner(AccessType::TransferWrite);

    assert!(src.info.usage.contains(vk::ImageUsageFlags::TRANSFER_SRC));
    assert!(dst.info.usage.contains(vk::ImageUsageFlags::TRANSFER_DST));

    // Get the raw vk handles
    let src = **src;
    let dst = **dst;

    // TODO: Maybe calculate subresource usage based on regions, it could add efficiency?
    let regions = regions.to_vec();

    cmd_chain
        .into()
        .push_shared_ref(src_binding.shared_ref())
        .push_shared_ref(dst_binding.shared_ref())
        .push_execute(move |device, cmd_buf| unsafe {
            CommandBuffer::image_barrier(
                cmd_buf,
                previous_src_access,
                AccessType::TransferRead,
                src,
                None,
            );
            CommandBuffer::image_barrier(
                cmd_buf,
                previous_dst_access,
                AccessType::TransferWrite,
                dst,
                None,
            );
            copy_image(device, **cmd_buf, src, dst, &regions);
        })
}

unsafe fn copy_image(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    src_image: vk::Image,
    dst_image: vk::Image,
    regions: &[vk::ImageCopy],
) {
    device.cmd_copy_image(
        cmd_buf,
        src_image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        dst_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        regions,
    );
}

pub fn copy_image_node<P>(
    render_graph: &mut RenderGraph<P>,
    src_node: impl Into<AnyImageNode<P>>,
    dst_node: impl Into<AnyImageNode<P>>,
) where
    P: SharedPointerKind + 'static,
{
    let src_node = src_node.into();
    let dst_node = dst_node.into();

    let src_info = render_graph.node_info(src_node);
    let dst_info = render_graph.node_info(dst_node);

    copy_image_node_region(
        render_graph,
        src_node,
        dst_node,
        &vk::ImageCopy {
            src_subresource: vk::ImageSubresourceLayers {
                aspect_mask: format_aspect_mask(src_info.fmt),
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            src_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            dst_subresource: vk::ImageSubresourceLayers {
                aspect_mask: format_aspect_mask(dst_info.fmt),
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            dst_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            extent: vk::Extent3D {
                depth: src_info.extent.z.min(dst_info.extent.z),
                height: src_info.extent.y.min(dst_info.extent.y),
                width: src_info.extent.x.min(dst_info.extent.x),
            },
        },
    );
}

pub fn copy_image_node_region<P>(
    render_graph: &mut RenderGraph<P>,
    src_node: impl Into<AnyImageNode<P>>,
    dst_node: impl Into<AnyImageNode<P>>,
    region: &vk::ImageCopy,
) where
    P: SharedPointerKind + 'static,
{
    use std::slice::from_ref;

    copy_image_node_regions(render_graph, src_node, dst_node, from_ref(region))
}

pub fn copy_image_node_regions<P>(
    render_graph: &mut RenderGraph<P>,
    src_node: impl Into<AnyImageNode<P>>,
    dst_node: impl Into<AnyImageNode<P>>,
    regions: &[vk::ImageCopy],
) where
    P: SharedPointerKind + 'static,
{
    let src_node = src_node.into();
    let dst_node = dst_node.into();

    assert!(render_graph
        .node_info(src_node)
        .usage
        .contains(vk::ImageUsageFlags::TRANSFER_SRC));
    assert!(render_graph
        .node_info(dst_node)
        .usage
        .contains(vk::ImageUsageFlags::TRANSFER_DST));

    let regions = regions.to_vec();

    render_graph
        .record_pass("copy image")
        .access_node(src_node, AccessType::TransferRead)
        .access_node(dst_node, AccessType::TransferWrite)
        .execute_pass(move |device, cmd_buf, bindings| unsafe {
            copy_image(
                device,
                cmd_buf,
                *bindings[src_node],
                *bindings[dst_node],
                &regions,
            );
        });
}
