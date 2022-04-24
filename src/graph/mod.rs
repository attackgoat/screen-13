mod binding;
mod edge;
mod info;
mod node;
mod pass_ref;
mod resolver;
mod swapchain;

pub use {
    self::{
        binding::{
            AnyBufferBinding, AnyImageBinding, Bind, BufferBinding, BufferLeaseBinding,
            ImageBinding, ImageLeaseBinding, RayTraceAccelerationBinding,
            RayTraceAccelerationLeaseBinding,
        },
        node::{
            AnyBufferNode, AnyImageNode, BufferLeaseNode, BufferNode, ImageLeaseNode, ImageNode,
            RayTraceAccelerationLeaseNode, RayTraceAccelerationNode, SwapchainImageNode, Unbind,
            View, ViewType,
        },
        pass_ref::{Bindings, Compute, Draw, PassRef, PipelinePassRef, RayTrace},
        resolver::Resolver,
        swapchain::SwapchainImageBinding,
    },
    vk_sync::AccessType,
};

use {
    self::{binding::Binding, edge::Edge, info::Information, node::Node},
    crate::{
        driver::{
            buffer_copy_subresources, buffer_image_copy_subresource, format_aspect_mask,
            is_write_access, BufferSubresource, ComputePipeline, DepthStencilMode,
            DescriptorBindingMap, GraphicPipeline, ImageSubresource, ImageType,
            PipelineDescriptorInfo, RayTracePipeline, SampleCount,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    ash::vk,
    std::{
        cmp::Ord,
        collections::{BTreeMap, BTreeSet},
        fmt::{Debug, Formatter},
        ops::Range,
    },
};

// Aliases for clarity
pub type AttachmentIndex = u32;
pub type BindingIndex = u32;
pub type BindingOffset = u32;
pub type DescriptorSetIndex = u32;

type ExecFn<P> = Box<dyn FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send>;
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
    fmt: vk::Format,
    sample_count: SampleCount,
    target: NodeIndex,
}

impl Attachment {
    fn are_compatible(lhs: Option<Self>, rhs: Option<Self>) -> bool {
        // Two attachment references are compatible if they have matching format and sample
        // count, or are both VK_ATTACHMENT_UNUSED or the pointer that would contain the
        // reference is NULL.
        if lhs.is_none() || rhs.is_none() {
            return true;
        }

        Self::are_identical(lhs.unwrap(), rhs.unwrap())
    }

    fn are_identical(lhs: Self, rhs: Self) -> bool {
        lhs.fmt == rhs.fmt && lhs.sample_count == rhs.sample_count
    }
}

#[derive(Clone, Debug, Default)]
struct AttachmentMap {
    attached: Vec<Option<Attachment>>,
    attached_count: usize,
    depth_stencil: Option<AttachmentIndex>,
}

impl AttachmentMap {
    fn attached(&self) -> impl Iterator<Item = AttachmentIndex> + '_ {
        self.attached
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| opt.map(|_| idx as AttachmentIndex))
    }

    fn are_compatible(&self, other: &Self) -> bool {
        // Count of the color attachments may differ, the extras are VK_ATTACHMENT_UNUSED
        self.attached
            .iter()
            .zip(other.attached.iter())
            .all(|(lhs, rhs)| Attachment::are_compatible(*lhs, *rhs))
    }

    fn contains_attachment(&self, attachment: AttachmentIndex) -> bool {
        self.attached.get(attachment as usize).is_some()
    }

    fn contains_image(&self, node_idx: NodeIndex) -> bool {
        self.attached
            .iter()
            .any(|attachment| matches!(attachment, Some(Attachment { target, .. }) if *target == node_idx))
    }

    fn depth_stencil(&self) -> Option<(AttachmentIndex, Attachment)> {
        self.depth_stencil.map(|attachment_idx| {
            (
                attachment_idx as AttachmentIndex,
                self.attached[attachment_idx as usize].unwrap(),
            )
        })
    }

    fn get(&self, attachment: AttachmentIndex) -> Option<Attachment> {
        self.attached.get(attachment as usize).copied().flatten()
    }

    /// Returns true if the previous attachment was compatible
    fn insert_color(
        &mut self,
        attachment: AttachmentIndex,
        aspect_mask: vk::ImageAspectFlags,
        fmt: vk::Format,
        sample_count: SampleCount,
        target: NodeIndex,
    ) -> bool {
        // Extend the data as needed
        self.extend_attached(attachment);

        if self.attached[attachment as usize].is_none() {
            self.attached_count += 1;
        }

        Self::set_attachment(
            &mut self.attached[attachment as usize],
            Attachment {
                aspect_mask,
                fmt,
                sample_count,
                target,
            },
        )
    }

    fn extend_attached(&mut self, attachment_idx: u32) {
        let attachment_count = attachment_idx as usize + 1;
        if attachment_count > self.attached.len() {
            self.attached
                .reserve(attachment_count - self.attached.len());
            while self.attached.len() < attachment_count {
                self.attached.push(None);
            }
        }
    }

    // Returns the unique targets of this instance.
    fn images(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        let mut already_seen = BTreeSet::new();
        self.attached
            .iter()
            .filter_map(|attachment| attachment.as_ref().map(|attachment| attachment.target))
            .filter(move |target| already_seen.insert(*target))
    }

    fn set_attachment(curr: &mut Option<Attachment>, next: Attachment) -> bool {
        curr.replace(next)
            .map(|curr| Attachment::are_identical(curr, next))
            .unwrap_or(true)
    }

    fn set_depth_stencil(
        &mut self,
        attachment: AttachmentIndex,
        aspect_mask: vk::ImageAspectFlags,
        fmt: vk::Format,
        sample_count: SampleCount,
        target: NodeIndex,
    ) -> bool {
        // Extend the data as needed
        self.extend_attached(attachment);

        assert!(self.depth_stencil.is_none());

        self.attached_count += 1;
        self.depth_stencil = Some(attachment);

        Self::set_attachment(
            &mut self.attached[attachment as usize],
            Attachment {
                aspect_mask,
                fmt,
                sample_count,
                target,
            },
        )
    }
}

pub struct Color(pub [f32; 4]);

impl From<[f32; 4]> for Color {
    fn from(color: [f32; 4]) -> Self {
        Self(color)
    }
}

impl From<[u8; 4]> for Color {
    fn from(color: [u8; 4]) -> Self {
        Self([
            color[0] as f32 / u8::MAX as f32,
            color[1] as f32 / u8::MAX as f32,
            color[2] as f32 / u8::MAX as f32,
            color[3] as f32 / u8::MAX as f32,
        ])
    }
}

/// Describes the SPIR-V binding index, and optionally a specific descriptor set
/// and array index.
///
/// Generally you might pass a function a descriptor using a simple integer:
///
/// ```rust
/// # fn my_func(_: usize, _: ()) {}
/// # let image = ();
/// let descriptor = 42;
/// my_func(descriptor, image);
/// ```
///
/// But also:
///
/// - `(0, 42)` for descriptor set `0` and binding index `42`
/// - `(42, [8])` for the same binding, but the 8th element
/// - `(0, 42, [8])` same as the previous example
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Descriptor {
    ArrayBinding(DescriptorSetIndex, BindingIndex, BindingOffset),
    Binding(DescriptorSetIndex, BindingIndex),
}

impl Descriptor {
    fn into_tuple(self) -> (DescriptorSetIndex, BindingIndex, BindingOffset) {
        match self {
            Self::ArrayBinding(descriptor_set_idx, binding_idx, binding_offset) => {
                (descriptor_set_idx, binding_idx, binding_offset)
            }
            Self::Binding(descriptor_set_idx, binding_idx) => (descriptor_set_idx, binding_idx, 0),
        }
    }

    fn set(self) -> DescriptorSetIndex {
        let (res, _, _) = self.into_tuple();
        res
    }
}

impl From<BindingIndex> for Descriptor {
    fn from(val: BindingIndex) -> Self {
        Self::Binding(0, val)
    }
}

impl From<(DescriptorSetIndex, BindingIndex)> for Descriptor {
    fn from(tuple: (DescriptorSetIndex, BindingIndex)) -> Self {
        Self::Binding(tuple.0, tuple.1)
    }
}

impl From<(BindingIndex, [BindingOffset; 1])> for Descriptor {
    fn from(tuple: (BindingIndex, [BindingOffset; 1])) -> Self {
        Self::ArrayBinding(0, tuple.0, tuple.1[0])
    }
}

impl From<(DescriptorSetIndex, BindingIndex, [BindingOffset; 1])> for Descriptor {
    fn from(tuple: (DescriptorSetIndex, BindingIndex, [BindingOffset; 1])) -> Self {
        Self::ArrayBinding(tuple.0, tuple.1, tuple.2[0])
    }
}

struct Execution<P>
where
    P: SharedPointerKind,
{
    accesses: BTreeMap<NodeIndex, [SubresourceAccess; 2]>,
    bindings: BTreeMap<Descriptor, (NodeIndex, Option<ViewType>)>,

    clears: BTreeMap<AttachmentIndex, vk::ClearValue>,
    loads: AttachmentMap,
    resolves: AttachmentMap,
    stores: AttachmentMap,

    func: Option<ExecutionFunction<P>>,
    pipeline: Option<ExecutionPipeline<P>>,
}

impl<P> Execution<P>
where
    P: SharedPointerKind,
{
    fn attachment(&self, attachment_idx: AttachmentIndex) -> Option<Attachment> {
        self.loads.get(attachment_idx).or_else(|| {
            self.resolves
                .get(attachment_idx)
                .or_else(|| self.stores.get(attachment_idx))
        })
    }

    fn attachment_count(&self) -> usize {
        self.loads
            .attached
            .len()
            .max(self.resolves.attached.len())
            .max(self.stores.attached.len())
    }

    #[cfg(debug_assertions)]
    fn attached_written(&self) -> impl Iterator<Item = AttachmentIndex> + '_ {
        self.clears
            .keys()
            .copied()
            .chain(self.resolves.attached())
            .chain(self.stores.attached())
    }
}

impl<P> Debug for Execution<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Execution")
    }
}

impl<P> Default for Execution<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            accesses: Default::default(),
            bindings: Default::default(),

            clears: Default::default(),
            loads: Default::default(),
            resolves: Default::default(),
            stores: Default::default(),

            func: None,
            pipeline: None,
        }
    }
}

struct ExecutionFunction<P>(ExecFn<P>)
where
    P: SharedPointerKind;

#[derive(Debug)]
enum ExecutionPipeline<P>
where
    P: SharedPointerKind,
{
    Compute(Shared<ComputePipeline<P>, P>),
    Graphic(Shared<GraphicPipeline<P>, P>),
    RayTrace(Shared<RayTracePipeline<P>, P>),
}

impl<P> ExecutionPipeline<P>
where
    P: SharedPointerKind,
{
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

    fn descriptor_info(&self) -> &PipelineDescriptorInfo<P> {
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

impl<P> Clone for ExecutionPipeline<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        match self {
            Self::Compute(pipeline) => Self::Compute(Shared::clone(pipeline)),
            Self::Graphic(pipeline) => Self::Graphic(Shared::clone(pipeline)),
            Self::RayTrace(pipeline) => Self::RayTrace(Shared::clone(pipeline)),
        }
    }
}

#[derive(Debug)]
struct Pass<P>
where
    P: SharedPointerKind,
{
    depth_stencil: Option<DepthStencilMode>,
    execs: Vec<Execution<P>>,
    name: String,
    render_area: Option<Area>,
}

impl<P> Pass<P>
where
    P: SharedPointerKind,
{
    fn descriptor_pools_sizes(
        &self,
    ) -> impl Iterator<Item = &BTreeMap<u32, BTreeMap<vk::DescriptorType, u32>>> {
        self.execs
            .iter()
            .flat_map(|exec| exec.pipeline.as_ref())
            .map(|pipeline| &pipeline.descriptor_info().pool_sizes)
    }
}

#[derive(Debug)]
pub struct RenderGraph<P>
where
    P: SharedPointerKind,
{
    bindings: Vec<Binding<P>>,
    passes: Vec<Pass<P>>,

    /// Set to true (when in debug mode) in order to get a breakpoint hit where you want.
    #[cfg(debug_assertions)]
    pub debug: bool,
}

impl<P> RenderGraph<P>
where
    P: SharedPointerKind + Send + 'static,
{
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

    pub fn bind_node<'a, B>(&'a mut self, binding: B) -> <B as Edge<Self>>::Result
    where
        B: Edge<Self>,
        B: Bind<&'a mut Self, <B as Edge<Self>>::Result, P>,
        P: 'static,
    {
        binding.bind(self)
    }

    pub fn blit_image(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
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

    pub fn blit_image_region(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
        region: &vk::ImageBlit,
        filter: vk::Filter,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.blit_image_regions(src_node, dst_node, from_ref(region), filter)
    }

    pub fn blit_image_regions(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
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

    /// Clears a color image as part of a render graph but outside of any graphic render pass
    pub fn clear_color_image(&mut self, image_node: impl Into<AnyImageNode<P>>) -> &mut Self
    where
        P: SharedPointerKind + 'static,
    {
        self.clear_color_image_value(image_node, [0, 0, 0, 0])
    }

    pub fn clear_color_image_value(
        &mut self,
        image_node: impl Into<AnyImageNode<P>>,
        color_value: impl Into<Color>,
    ) -> &mut Self
    where
        P: SharedPointerKind + 'static,
    {
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

    /// Clears a depth/stencil image as part of a render graph but outside of any graphic render pass
    pub fn clear_depth_stencil_image(&mut self, image_node: impl Into<AnyImageNode<P>>) -> &mut Self
    where
        P: SharedPointerKind + 'static,
    {
        self.clear_depth_stencil_image_value(image_node, 0.0, 0)
    }

    pub fn clear_depth_stencil_image_value(
        &mut self,
        image_node: impl Into<AnyImageNode<P>>,
        depth: f32,
        stencil: u32,
    ) -> &mut Self
    where
        P: SharedPointerKind + 'static,
    {
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

    pub fn copy_buffer(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
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

    pub fn copy_buffer_region(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
        region: &vk::BufferCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_buffer_regions(src_node, dst_node, from_ref(region))
    }

    pub fn copy_buffer_regions(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
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

    pub fn copy_buffer_to_image(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
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

    pub fn copy_buffer_to_image_region(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
        region: &vk::BufferImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_buffer_to_image_regions(src_node, dst_node, from_ref(region))
    }

    pub fn copy_buffer_to_image_regions(
        &mut self,
        src_node: impl Into<AnyBufferNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
        regions: impl Into<Box<[vk::BufferImageCopy]>>,
    ) -> &mut Self {
        let src_node = src_node.into();
        let dst_node = dst_node.into();
        let dst_access_range = self.node_info(dst_node).default_view_info();
        let regions = regions.into();
        let src_access_range = buffer_image_copy_subresource(&regions);

        self.begin_pass("copy image")
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

    pub fn copy_image(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
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

    pub fn copy_image_region(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
        region: &vk::ImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_image_regions(src_node, dst_node, from_ref(region))
    }

    pub fn copy_image_regions(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyImageNode<P>>,
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

    pub fn copy_image_to_buffer(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
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

    pub fn copy_image_to_buffer_region(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
        region: &vk::BufferImageCopy,
    ) -> &mut Self {
        use std::slice::from_ref;

        self.copy_image_to_buffer_regions(src_node, dst_node, from_ref(region))
    }

    pub fn copy_image_to_buffer_regions(
        &mut self,
        src_node: impl Into<AnyImageNode<P>>,
        dst_node: impl Into<AnyBufferNode<P>>,
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

    pub fn fill_buffer(
        &mut self,
        buffer_node: impl Into<AnyBufferNode<P>>,
        data: u32,
    ) -> &mut Self {
        let buffer_node = buffer_node.into();

        let buffer_info = self.node_info(buffer_node);

        self.fill_buffer_region(buffer_node, data, 0..buffer_info.size)
    }

    pub fn fill_buffer_region(
        &mut self,
        buffer_node: impl Into<AnyBufferNode<P>>,
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
    fn first_node_access_pass_index(&self, node: impl Node<P>) -> Option<usize> {
        self.node_access_pass_index(node, self.passes.iter())
    }

    pub(super) fn last_access(&self, node: impl Node<P>) -> Option<AccessType> {
        let node_idx = node.index();

        self.passes
            .iter()
            .rev()
            .flat_map(|pass| pass.execs.iter().rev())
            .find_map(|exec| {
                exec.accesses
                    .get(&node_idx)
                    .map(|[_early, late]| late.access)
            })
    }

    pub(super) fn last_write(&self, node: impl Node<P>) -> Option<AccessType> {
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
        node: impl Node<P>,
        passes: impl Iterator<Item = &'a Pass<P>>,
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

    pub fn node_info<N>(&self, node: N) -> <N as Information>::Info
    where
        N: Information,
    {
        node.get(self)
    }

    pub fn begin_pass(&mut self, name: impl AsRef<str>) -> PassRef<'_, P> {
        PassRef::new(self, name.as_ref().to_string())
    }

    pub fn resolve(mut self) -> Resolver<P> {
        // The final execution of each pass has no function
        for pass in &mut self.passes {
            pass.execs.pop();
        }

        Resolver::new(self)
    }

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
        buffer_node: impl Into<AnyBufferNode<P>>,
        data: &'static [u8],
    ) -> &mut Self {
        self.update_buffer_offset(buffer_node, data, 0)
    }

    /// Note: `data` must not exceed 65536 bytes.
    pub fn update_buffer_offset(
        &mut self,
        buffer_node: impl Into<AnyBufferNode<P>>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subresource {
    Image(ImageSubresource),
    Buffer(BufferSubresource),
}

impl Subresource {
    fn unwrap_buffer(self) -> BufferSubresource {
        if let Self::Buffer(subresource) = self {
            subresource
        } else {
            unreachable!();
        }
    }

    fn unwrap_image(self) -> ImageSubresource {
        if let Self::Image(subresource) = self {
            subresource
        } else {
            unreachable!();
        }
    }
}

impl From<ImageSubresource> for Subresource {
    fn from(subresource: ImageSubresource) -> Self {
        Self::Image(subresource)
    }
}

impl From<BufferSubresource> for Subresource {
    fn from(subresource: BufferSubresource) -> Self {
        Self::Buffer(subresource)
    }
}

#[derive(Clone, Copy, Debug)]
struct SubresourceAccess {
    access: AccessType,
    subresource: Option<Subresource>,
}
