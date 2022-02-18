mod binding;
mod edge;
mod info;
mod node;
mod pass_ref;
mod resolver;
mod swapchain;
mod validator;

// Re-imports
pub use vk_sync::{AccessType, ImageLayout};

pub use self::{
    binding::{
        AnyBufferBinding, AnyImageBinding, Bind, BufferBinding, BufferLeaseBinding,
        DescriptorPoolBinding, ImageBinding, ImageLeaseBinding, RayTraceAccelerationBinding,
        RayTraceAccelerationLeaseBinding, RenderPassBinding,
    },
    node::{
        AnyBufferNode, AnyImageNode, BufferLeaseNode, BufferNode, ImageLeaseNode, ImageNode,
        RayTraceAccelerationLeaseNode, RayTraceAccelerationNode, SwapchainImageNode, Unbind, View,
        ViewType,
    },
    pass_ref::{Bindings, PassRef, PipelinePassRef},
    resolver::Resolver,
    swapchain::SwapchainImageBinding,
};

use {
    self::{binding::Binding, edge::Edge, info::Information, node::Node},
    crate::{
        driver::{
            BufferSubresource, ComputePipeline, DepthStencilMode, DescriptorBindingMap,
            DescriptorInfo, DescriptorSetLayout, GraphicPipeline, ImageSubresource,
            PipelineDescriptorInfo, RayTracePipeline, SampleCount,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    ash::vk,
    glam::{IVec2, UVec2, Vec2},
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

type ExecFn<P> = Box<dyn FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>)>;
type NodeIndex = usize;

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

#[derive(Debug, Default)]
struct AttachmentMap {
    attached: Vec<Option<Attachment>>,
    attached_count: usize,
    depth_stencil: Option<AttachmentIndex>,
}

impl AttachmentMap {
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

    // fn with_capacity(capacity: usize) -> Self {
    //     Self {
    //         attached: Vec::with_capacity(capacity),
    //         ..Default::default()
    //     }
    // }
}

// TODO: Now maybe don't need this with spirq's DescriptorBinding?
/// Describes the SPIRV binding index, and optionally a specific descriptor set
/// and array index.
///
/// Generally you might pass a function a descriptor using a simple integer:
///
/// ```rust
/// let descriptor = 42;
/// my_function(descriptor, image);
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

#[derive(Debug)]
struct NodeAccess {
    node_idx: NodeIndex,
    ty: AccessType,
    subresource: Option<Subresource>,
}

struct Execution<P>
where
    P: SharedPointerKind,
{
    accesses: Vec<NodeAccess>,
    bindings: BTreeMap<Descriptor, (NodeIndex, Option<ViewType>)>,
    clears: BTreeMap<AttachmentIndex, vk::ClearValue>,
    func: Option<ExecutionFunction<P>>,
    pipeline: Option<ExecutionPipeline<P>>,
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
            accesses: vec![],
            bindings: Default::default(),
            clears: Default::default(),
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
            ExecutionPipeline::Compute(pipeline) => vk::PipelineStageFlags::COMPUTE_SHADER,
            ExecutionPipeline::Graphic(pipeline) => vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ExecutionPipeline::RayTrace(pipeline) => vk::PipelineStageFlags::RAY_TRACING_SHADER_KHR,
        }
    }
}

#[derive(Debug)]
struct Pass<P>
where
    P: SharedPointerKind,
{
    load_attachments: AttachmentMap,
    resolve_attachments: AttachmentMap,
    store_attachments: AttachmentMap,
    depth_stencil: Option<DepthStencilMode>,
    execs: Vec<Execution<P>>,
    name: String,
    push_consts: Vec<PushConstantRange>,
    render_area: Option<Rect<UVec2, IVec2>>,
    scissor: Option<Rect<UVec2, IVec2>>,
    subpasses: Vec<Subpass>,
    viewport: Option<(Rect<Vec2, Vec2>, Range<f32>)>,
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
struct PushConstantRange {
    data: Vec<u8>,
    offset: u32,
    stage: vk::ShaderStageFlags,
}

#[derive(Clone, Copy, Debug)]
struct Rect<E, O> {
    extent: E,
    offset: O,
}

#[derive(Debug)]
pub struct RenderGraph<P>
where
    P: SharedPointerKind,
{
    bindings: Vec<Binding<P>>,
    passes: Vec<Pass<P>>,
}

impl<P> RenderGraph<P>
where
    P: SharedPointerKind + 'static,
{
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            bindings: vec![],
            passes: vec![],
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

    /// Returns the index of the first pass which accesses a given node
    fn first_node_access_pass_index(&self, node: impl Node<P>) -> Option<usize> {
        self.node_access_pass_index(node, self.passes.iter())
    }

    pub(super) fn last_access(
        &self,
        node: impl Node<P>,
    ) -> Option<(AccessType, Option<Subresource>)> {
        let node_idx = node.index();

        self.passes
            .iter()
            .rev()
            .flat_map(|pass| pass.execs.iter().rev())
            .flat_map(|exec| exec.accesses.iter())
            .find(|access| access.node_idx == node_idx)
            .map(|access| (access.ty, access.subresource.clone()))
    }

    /// Returns the index of the last pass which accesses a given node
    fn last_node_access_pass_index(&self, node: impl Node<P>) -> Option<usize> {
        self.node_access_pass_index(node, self.passes.iter().rev())
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
                for access in exec.accesses.iter() {
                    if access.node_idx == node_idx {
                        return Some(pass_idx);
                    }
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

    pub fn record_pass(&mut self, name: impl AsRef<str>) -> PassRef<'_, P> {
        PassRef::new(self, name.as_ref().to_string())
    }

    pub fn resolve(self) -> Resolver<P> {
        Resolver::new(self)
    }

    pub fn unbind_node<N>(&mut self, node: N) -> <N as Edge<Self>>::Result
    where
        N: Edge<Self>,
        N: Unbind<Self, <N as Edge<Self>>::Result>,
    {
        node.unbind(self)
    }
}

#[derive(Debug)]
struct Subpass {
    exec_idx: usize,
    load_attachments: AttachmentMap,
    resolve_attachments: AttachmentMap,
    store_attachments: AttachmentMap,
}

impl Subpass {
    fn attachment(&self, attachment_idx: AttachmentIndex) -> Option<Attachment> {
        self.load_attachments.get(attachment_idx).or_else(|| {
            self.resolve_attachments
                .get(attachment_idx)
                .or_else(|| self.store_attachments.get(attachment_idx))
        })
    }

    fn attachment_count(&self) -> usize {
        self.load_attachments
            .attached
            .len()
            .max(self.resolve_attachments.attached.len())
            .max(self.store_attachments.attached.len())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
