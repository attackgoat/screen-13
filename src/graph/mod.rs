//! Rendering operations and command submission.
//!
//!

pub mod node;
pub mod pass_ref;

mod binding;
mod edge;
mod info;
mod resolver;
mod swapchain;

pub use self::{
    binding::{Bind, Unbind},
    resolver::{Resolver, ResolverPool},
};

use {
    self::{
        binding::Binding,
        edge::Edge,
        info::Information,
        node::Node,
        node::{
            AccelerationStructureLeaseNode, AccelerationStructureNode,
            AnyAccelerationStructureNode, AnyBufferNode, AnyImageNode, BufferLeaseNode, BufferNode,
            ImageLeaseNode, ImageNode, SwapchainImageNode,
        },
        pass_ref::{AttachmentIndex, Bindings, Descriptor, PassRef, SubresourceAccess, ViewType},
    },
    crate::driver::{
        buffer_copy_subresources, buffer_image_copy_subresource,
        compute::ComputePipeline,
        format_aspect_mask,
        graphic::{DepthStencilMode, GraphicPipeline},
        image::{ImageType, SampleCount},
        is_write_access,
        ray_trace::RayTracePipeline,
        shader::PipelineDescriptorInfo,
        DescriptorBindingMap, Device,
    },
    ash::vk,
    std::{
        cmp::Ord,
        collections::HashMap,
        fmt::{Debug, Formatter},
        ops::Range,
        sync::Arc,
    },
    vk_sync::AccessType,
};

type ExecFn = Box<dyn FnOnce(&Device, vk::CommandBuffer, Bindings<'_>) + Send>;
type NodeIndex = usize;

#[derive(Clone, Copy, Debug)]
struct Area {
    height: u32,
    width: u32,
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Attachment {
    aspect_mask: vk::ImageAspectFlags,
    format: vk::Format,
    sample_count: SampleCount,
    target: NodeIndex,
}

// impl Attachment {
//     fn are_compatible(lhs: Option<Self>, rhs: Option<Self>) -> bool {
//         // Two attachment references are compatible if they have matching format and sample
//         // count, or are both VK_ATTACHMENT_UNUSED or the pointer that would contain the
//         // reference is NULL.
//         if lhs.is_none() || rhs.is_none() {
//             return true;
//         }

//         Self::are_identical(lhs.unwrap(), rhs.unwrap())
//     }

//     pub fn are_identical(lhs: Self, rhs: Self) -> bool {
//         lhs.fmt == rhs.fmt && lhs.sample_count == rhs.sample_count && lhs.target == rhs.target
//     }
// }

/// Specifies a color attachment clear value which can be used to initliaze an image.
#[derive(Clone, Copy, Debug)]
pub struct ClearColorValue(pub [f32; 4]);

impl From<[f32; 4]> for ClearColorValue {
    fn from(color: [f32; 4]) -> Self {
        Self(color)
    }
}

impl From<[u8; 4]> for ClearColorValue {
    fn from(color: [u8; 4]) -> Self {
        Self([
            color[0] as f32 / u8::MAX as f32,
            color[1] as f32 / u8::MAX as f32,
            color[2] as f32 / u8::MAX as f32,
            color[3] as f32 / u8::MAX as f32,
        ])
    }
}

#[derive(Default)]
struct Execution {
    accesses: HashMap<NodeIndex, [SubresourceAccess; 2]>,
    bindings: HashMap<Descriptor, (NodeIndex, Option<ViewType>)>,

    depth_stencil: Option<DepthStencilMode>,

    color_attachments: HashMap<AttachmentIndex, Attachment>,
    color_clears: HashMap<AttachmentIndex, (Attachment, ClearColorValue)>,
    color_loads: HashMap<AttachmentIndex, Attachment>,
    color_resolves: HashMap<AttachmentIndex, (Attachment, AttachmentIndex)>,
    color_stores: HashMap<AttachmentIndex, Attachment>,
    depth_stencil_attachment: Option<Attachment>,
    depth_stencil_clear: Option<(Attachment, vk::ClearDepthStencilValue)>,
    depth_stencil_load: Option<Attachment>,
    depth_stencil_resolve: Option<Attachment>,
    depth_stencil_store: Option<Attachment>,

    func: Option<ExecutionFunction>,
    pipeline: Option<ExecutionPipeline>,
}

impl Debug for Execution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // The only field missing is func which cannot easily be implemented because it is a
        // FnOnce.
        f.debug_struct("Execution")
            .field("accesses", &self.accesses)
            .field("bindings", &self.bindings)
            .field("depth_stencil", &self.depth_stencil)
            .field("color_attachments", &self.color_attachments)
            .field("color_clears", &self.color_clears)
            .field("color_loads", &self.color_loads)
            .field("color_resolves", &self.color_resolves)
            .field("color_stores", &self.color_stores)
            .field("depth_stencil_attachment", &self.depth_stencil_attachment)
            .field("depth_stencil_clear", &self.depth_stencil_clear)
            .field("depth_stencil_load", &self.depth_stencil_load)
            .field("depth_stencil_resolve", &self.depth_stencil_resolve)
            .field("depth_stencil_store", &self.depth_stencil_store)
            .field("pipeline", &self.pipeline)
            .finish()
    }
}

struct ExecutionFunction(ExecFn);

#[derive(Debug)]
enum ExecutionPipeline {
    Compute(Arc<ComputePipeline>),
    Graphic(Arc<GraphicPipeline>),
    RayTrace(Arc<RayTracePipeline>),
}

impl ExecutionPipeline {
    fn bind_point(&self) -> vk::PipelineBindPoint {
        match self {
            ExecutionPipeline::Compute(_) => vk::PipelineBindPoint::COMPUTE,
            ExecutionPipeline::Graphic(_) => vk::PipelineBindPoint::GRAPHICS,
            ExecutionPipeline::RayTrace(_) => vk::PipelineBindPoint::RAY_TRACING_KHR,
        }
    }

    fn descriptor_bindings(&self) -> &DescriptorBindingMap {
        match self {
            ExecutionPipeline::Compute(pipeline) => &pipeline.descriptor_bindings,
            ExecutionPipeline::Graphic(pipeline) => &pipeline.descriptor_bindings,
            ExecutionPipeline::RayTrace(pipeline) => &pipeline.descriptor_bindings,
        }
    }

    fn descriptor_info(&self) -> &PipelineDescriptorInfo {
        match self {
            ExecutionPipeline::Compute(pipeline) => &pipeline.descriptor_info,
            ExecutionPipeline::Graphic(pipeline) => &pipeline.descriptor_info,
            ExecutionPipeline::RayTrace(pipeline) => &pipeline.descriptor_info,
        }
    }

    fn layout(&self) -> vk::PipelineLayout {
        match self {
            ExecutionPipeline::Compute(pipeline) => pipeline.layout,
            ExecutionPipeline::Graphic(pipeline) => pipeline.layout,
            ExecutionPipeline::RayTrace(pipeline) => pipeline.layout,
        }
    }

    fn stage(&self) -> vk::PipelineStageFlags {
        match self {
            ExecutionPipeline::Compute(_) => vk::PipelineStageFlags::COMPUTE_SHADER,
            ExecutionPipeline::Graphic(_) => vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ExecutionPipeline::RayTrace(_) => vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
        }
    }
}

impl Clone for ExecutionPipeline {
    fn clone(&self) -> Self {
        match self {
            Self::Compute(pipeline) => Self::Compute(Arc::clone(pipeline)),
            Self::Graphic(pipeline) => Self::Graphic(Arc::clone(pipeline)),
            Self::RayTrace(pipeline) => Self::RayTrace(Arc::clone(pipeline)),
        }
    }
}

#[derive(Debug)]
struct Pass {
    execs: Vec<Execution>,
    name: String,
    render_area: Option<Area>,
}

impl Pass {
    fn descriptor_pools_sizes(
        &self,
    ) -> impl Iterator<Item = &HashMap<u32, HashMap<vk::DescriptorType, u32>>> {
        self.execs
            .iter()
            .flat_map(|exec| exec.pipeline.as_ref())
            .map(|pipeline| &pipeline.descriptor_info().pool_sizes)
    }
}

/// A composable graph of render pass operations.
///
/// `RenderGraph` instances are are intended for one-time use.
///
/// The design of this code originated with a combination of
/// [`PassBuilder`](https://github.com/EmbarkStudios/kajiya/blob/main/crates/lib/kajiya-rg/src/pass_builder.rs)
/// and
/// [`render_graph.cpp`](https://github.com/Themaister/Granite/blob/master/renderer/render_graph.cpp).
#[derive(Debug)]
pub struct RenderGraph {
    bindings: Vec<Binding>,
    passes: Vec<Pass>,

    /// Set to true (when in debug mode) in order to get a breakpoint hit where you want.
    #[cfg(debug_assertions)]
    pub debug: bool,
}

impl RenderGraph {
    /// Constructs a new `RenderGraph`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let bindings = vec![];
        let passes = vec![];

        #[cfg(debug_assertions)]
        let debug = false;

        Self {
            bindings,
            passes,
            #[cfg(debug_assertions)]
            debug,
        }
    }

    /// Begins a new pass.
    pub fn begin_pass(&mut self, name: impl AsRef<str>) -> PassRef<'_> {
        PassRef::new(self, name.as_ref().to_string())
    }

    /// Binds a Vulkan acceleration structure, buffer, or image to this graph.
    ///
    /// Bound nodes may be used in passes for pipeline and shader operations.
    pub fn bind_node<'a, B>(&'a mut self, binding: B) -> <B as Edge<Self>>::Result
    where
        B: Edge<Self>,
        B: Bind<&'a mut Self, <B as Edge<Self>>::Result>,
    {
        binding.bind(self)
    }

    /// Copy an image, potentially performing format conversion.
    pub fn blit_image(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
        filter: vk::Filter,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();

        let src_info = self.node_info(src_node);
        let dst_info = self.node_info(dst_node);

        self.blit_image_region(
            src_node,
            dst_node,
            &vk::ImageBlit {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(src_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [
                    vk::Offset3D {
                        x: src_info.width as _,
                        y: src_info.height as _,
                        z: src_info.depth as _,
                    },
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                ],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(dst_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_offsets: [
                    vk::Offset3D {
                        x: dst_info.width as _,
                        y: dst_info.height as _,
                        z: dst_info.depth as _,
                    },
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                ],
            },
            filter,
        )
    }

    /// Copy a region of an image, potentially performing format conversion.
    pub fn blit_image_region(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
        region: &vk::ImageBlit,
        filter: vk::Filter,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.blit_image_regions(src_node, dst_node, from_ref(region), filter)
    }

    /// Copy regions of an image, potentially performing format conversion.
    pub fn blit_image_regions(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
        regions: impl Into<Box<[vk::ImageBlit]>>,
        filter: vk::Filter,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let regions = regions.into();
        let src_access_range = self.node_info(src_node).default_view_info();
        let dst_access_range = self.node_info(dst_node).default_view_info();

        self.begin_pass("blit image")
            .access_node_subrange(src_node, AccessType::TransferRead, src_access_range)
            .access_node_subrange(dst_node, AccessType::TransferWrite, dst_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_blit_image(
                    cmd_buf,
                    *bindings[src_node],
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    *bindings[dst_node],
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    &regions,
                    filter,
                );
            })
            .submit_pass()
    }

    /// Clear a color image.
    pub fn clear_color_image(&mut self, image_node: impl Into<AnyImageNode>) -> &mut Self {
        self.clear_color_image_value(image_node, [0, 0, 0, 0])
    }

    /// Clear a color image.
    pub fn clear_color_image_value(
        &mut self,
        image_node: impl Into<AnyImageNode>,
        color_value: impl Into<ClearColorValue>,
    ) -> &mut Self {
        let color_value = color_value.into();
        let image_node = image_node.into();
        let image_info = self.node_info(image_node);
        let image_access_range = image_info.default_view_info();

        self.begin_pass("clear color")
            .access_node_subrange(image_node, AccessType::TransferWrite, image_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_clear_color_image(
                    cmd_buf,
                    *bindings[image_node],
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &vk::ClearColorValue {
                        float32: color_value.0,
                    },
                    &[vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: image_info.mip_level_count,
                        layer_count: image_info.array_elements,
                        ..Default::default()
                    }],
                );
            })
            .submit_pass()
    }

    /// Clears a depth/stencil image.
    pub fn clear_depth_stencil_image(&mut self, image_node: impl Into<AnyImageNode>) -> &mut Self {
        self.clear_depth_stencil_image_value(image_node, 1.0, 0)
    }

    /// Clears a depth/stencil image.
    pub fn clear_depth_stencil_image_value(
        &mut self,
        image_node: impl Into<AnyImageNode>,
        depth: f32,
        stencil: u32,
    ) -> &mut Self {
        let image_node = image_node.into();
        let image_info = self.node_info(image_node);
        let image_access_range = image_info.default_view_info();

        self.begin_pass("clear depth/stencil")
            .access_node_subrange(image_node, AccessType::TransferWrite, image_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_clear_depth_stencil_image(
                    cmd_buf,
                    *bindings[image_node],
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &vk::ClearDepthStencilValue { depth, stencil },
                    &[vk::ImageSubresourceRange {
                        aspect_mask: format_aspect_mask(image_info.fmt),
                        level_count: image_info.mip_level_count,
                        layer_count: image_info.array_elements,
                        ..Default::default()
                    }],
                );
            })
            .submit_pass()
    }

    /// Copy data between buffers
    pub fn copy_buffer(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyBufferNode>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let src_info = self.node_info(src_node);
        let dst_info = self.node_info(dst_node);

        self.copy_buffer_region(
            src_node,
            dst_node,
            &vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: src_info.size.min(dst_info.size),
            },
        )
    }

    /// Copy data between buffer regions.
    pub fn copy_buffer_region(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyBufferNode>,
        region: &vk::BufferCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_buffer_regions(src_node, dst_node, from_ref(region))
    }

    /// Copy data between buffer regions.
    pub fn copy_buffer_regions(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyBufferNode>,
        regions: impl Into<Box<[vk::BufferCopy]>>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let regions: Box<[_]> = regions.into();
        let (src_access_range, dst_access_range) = buffer_copy_subresources(&regions);

        self.begin_pass("copy buffer")
            .access_node_subrange(src_node, AccessType::TransferRead, src_access_range)
            .access_node_subrange(dst_node, AccessType::TransferWrite, dst_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_copy_buffer(cmd_buf, *bindings[src_node], *bindings[dst_node], &regions);
            })
            .submit_pass()
    }

    /// Copy data from a buffer into an image.
    pub fn copy_buffer_to_image(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyImageNode>,
    ) -> &mut Self {
        let dst_node = dst_node.into();
        let dst_info = self.node_info(dst_node);

        self.copy_buffer_to_image_region(
            src_node,
            dst_node,
            &vk::BufferImageCopy {
                buffer_offset: 0,
                buffer_row_length: dst_info.width,
                buffer_image_height: dst_info.height,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(dst_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_offset: Default::default(),
                image_extent: vk::Extent3D {
                    depth: dst_info.depth,
                    height: dst_info.height,
                    width: dst_info.width,
                },
            },
        )
    }

    /// Copy data from a buffer into an image.
    pub fn copy_buffer_to_image_region(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyImageNode>,
        region: &vk::BufferImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_buffer_to_image_regions(src_node, dst_node, from_ref(region))
    }

    /// Copy data from a buffer into an image.
    pub fn copy_buffer_to_image_regions(
        &mut self,
        src_node: impl Into<AnyBufferNode>,
        dst_node: impl Into<AnyImageNode>,
        regions: impl Into<Box<[vk::BufferImageCopy]>>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let dst_access_range = self.node_info(dst_node).default_view_info();
        let regions = regions.into();
        let src_access_range = buffer_image_copy_subresource(&regions);

        self.begin_pass("copy buffer to image")
            .access_node_subrange(src_node, AccessType::TransferRead, src_access_range)
            .access_node_subrange(dst_node, AccessType::TransferWrite, dst_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_copy_buffer_to_image(
                    cmd_buf,
                    *bindings[src_node],
                    *bindings[dst_node],
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                );
            })
            .submit_pass()
    }

    /// Copy data between images.
    pub fn copy_image(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();

        let src_info = self.node_info(src_node);
        let dst_info = self.node_info(dst_node);

        self.copy_image_region(
            src_node,
            dst_node,
            &vk::ImageCopy {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(src_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: if matches!(src_info.ty, ImageType::Cube | ImageType::CubeArray) {
                        6
                    } else {
                        1
                    },
                },
                src_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(dst_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: if matches!(dst_info.ty, ImageType::Cube | ImageType::CubeArray) {
                        6
                    } else {
                        1
                    },
                },
                dst_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                extent: vk::Extent3D {
                    depth: src_info.depth.min(dst_info.depth).max(1),
                    height: src_info.height.min(dst_info.height).max(1),
                    width: src_info.width.min(dst_info.width),
                },
            },
        )
    }

    /// Copy data between images.
    pub fn copy_image_region(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
        region: &vk::ImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_image_regions(src_node, dst_node, from_ref(region))
    }

    /// Copy data between images.
    pub fn copy_image_regions(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyImageNode>,
        regions: impl Into<Box<[vk::ImageCopy]>>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let src_access_range = self.node_info(src_node).default_view_info();
        let dst_access_range = self.node_info(dst_node).default_view_info();
        let regions = regions.into();

        self.begin_pass("copy image")
            .access_node_subrange(src_node, AccessType::TransferRead, src_access_range)
            .access_node_subrange(dst_node, AccessType::TransferWrite, dst_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_copy_image(
                    cmd_buf,
                    *bindings[src_node],
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    *bindings[dst_node],
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                );
            })
            .submit_pass()
    }

    /// Copy image data into a buffer.
    pub fn copy_image_to_buffer(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyBufferNode>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();

        let src_info = self.node_info(src_node);

        self.copy_image_to_buffer_region(
            src_node,
            dst_node,
            &vk::BufferImageCopy {
                buffer_offset: 0,
                buffer_row_length: src_info.width,
                buffer_image_height: src_info.height,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: format_aspect_mask(src_info.fmt),
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_offset: Default::default(),
                image_extent: vk::Extent3D {
                    depth: src_info.depth,
                    height: src_info.height,
                    width: src_info.width,
                },
            },
        )
    }

    /// Copy image data into a buffer.
    pub fn copy_image_to_buffer_region(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyBufferNode>,
        region: &vk::BufferImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_image_to_buffer_regions(src_node, dst_node, from_ref(region))
    }

    /// Copy image data into a buffer.
    pub fn copy_image_to_buffer_regions(
        &mut self,
        src_node: impl Into<AnyImageNode>,
        dst_node: impl Into<AnyBufferNode>,
        regions: impl Into<Box<[vk::BufferImageCopy]>>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let regions = regions.into();
        let src_subresource = self.node_info(src_node).default_view_info();
        let dst_subresource = buffer_image_copy_subresource(&regions);

        self.begin_pass("copy image to buffer")
            .access_node_subrange(src_node, AccessType::TransferRead, src_subresource)
            .access_node_subrange(dst_node, AccessType::TransferWrite, dst_subresource)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_copy_image_to_buffer(
                    cmd_buf,
                    *bindings[src_node],
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    *bindings[dst_node],
                    &regions,
                );
            })
            .submit_pass()
    }

    /// Fill a region of a buffer with a fixed value.
    pub fn fill_buffer(&mut self, buffer_node: impl Into<AnyBufferNode>, data: u32) -> &mut Self {
        let buffer_node = buffer_node.into();

        let buffer_info = self.node_info(buffer_node);

        self.fill_buffer_region(buffer_node, data, 0..buffer_info.size)
    }

    /// Fill a region of a buffer with a fixed value.
    pub fn fill_buffer_region(
        &mut self,
        buffer_node: impl Into<AnyBufferNode>,
        data: u32,
        region: Range<vk::DeviceSize>,
    ) -> &mut Self {
        let buffer_node = buffer_node.into();
        let buffer_info = self.node_info(buffer_node);
        let buffer_access_range = 0..buffer_info.size;

        self.begin_pass("fill buffer")
            .access_node_subrange(buffer_node, AccessType::TransferWrite, buffer_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_fill_buffer(
                    cmd_buf,
                    *bindings[buffer_node],
                    region.start,
                    region.end - region.start,
                    data,
                );
            })
            .submit_pass()
    }

    /// Returns the index of the first pass which accesses a given node
    fn first_node_access_pass_index(&self, node: impl Node) -> Option<usize> {
        self.node_access_pass_index(node, self.passes.iter())
    }

    pub(super) fn last_write(&self, node: impl Node) -> Option<AccessType> {
        let node_idx = node.index();

        self.passes
            .iter()
            .rev()
            .flat_map(|pass| pass.execs.iter().rev())
            .find_map(|exec| {
                exec.accesses.get(&node_idx).and_then(|[_early, late]| {
                    if is_write_access(late.access) {
                        Some(late.access)
                    } else {
                        None
                    }
                })
            })
    }

    /// Returns the index of the first pass in a list of passes which accesses a given node
    fn node_access_pass_index<'a>(
        &self,
        node: impl Node,
        passes: impl Iterator<Item = &'a Pass>,
    ) -> Option<usize> {
        let node_idx = node.index();

        for (pass_idx, pass) in passes.enumerate() {
            for exec in pass.execs.iter() {
                if exec.accesses.contains_key(&node_idx) {
                    return Some(pass_idx);
                }
            }
        }

        None
    }

    /// Returns information used to crate a node.
    pub fn node_info<N>(&self, node: N) -> <N as Information>::Info
    where
        N: Information,
    {
        node.get(self)
    }

    /// Finalizes the graph and provides an object with functions for submitting the resulting
    /// commands.
    pub fn resolve(mut self) -> Resolver {
        // The final execution of each pass has no function
        for pass in &mut self.passes {
            pass.execs.pop();
        }

        Resolver::new(self)
    }

    /// Removes a node from this graph.
    ///
    /// Future access to `node` on this graph will return invalid results.
    pub fn unbind_node<N>(&mut self, node: N) -> <N as Edge<Self>>::Result
    where
        N: Edge<Self>,
        N: Unbind<Self, <N as Edge<Self>>::Result>,
    {
        node.unbind(self)
    }

    /// Note: `data` must not exceed 65536 bytes.
    pub fn update_buffer(
        &mut self,
        buffer_node: impl Into<AnyBufferNode>,
        data: &'static [u8],
    ) -> &mut Self {
        self.update_buffer_offset(buffer_node, data, 0)
    }

    /// Note: `data` must not exceed 65536 bytes.
    pub fn update_buffer_offset(
        &mut self,
        buffer_node: impl Into<AnyBufferNode>,
        data: &'static [u8],
        offset: vk::DeviceSize,
    ) -> &mut Self {
        let buffer_node = buffer_node.into();
        let buffer_info = self.node_info(buffer_node);
        let buffer_access_range = 0..buffer_info.size;

        self.begin_pass("update buffer")
            .access_node_subrange(buffer_node, AccessType::TransferWrite, buffer_access_range)
            .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
                device.cmd_update_buffer(cmd_buf, *bindings[buffer_node], offset, data);
            })
            .submit_pass()
    }
}
