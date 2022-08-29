use {
    super::{Information, NodeIndex, RenderGraph, Subresource},
    crate::{
        driver::{
            vk, AccelerationStructure, AccelerationStructureInfo, Buffer, BufferInfo,
            BufferSubresource, Image, ImageInfo, ImageSubresource, ImageViewInfo,
        },
        pool::Lease,
    },
    std::{ops::Range, sync::Arc},
};

#[derive(Debug)]
pub enum AnyAccelerationStructureNode {
    AccelerationStructure(AccelerationStructureNode),
    AccelerationStructureLease(AccelerationStructureLeaseNode),
}

impl Clone for AnyAccelerationStructureNode {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for AnyAccelerationStructureNode {}

impl Information for AnyAccelerationStructureNode {
    type Info = AccelerationStructureInfo;

    fn get(self, graph: &RenderGraph) -> Self::Info {
        match self {
            Self::AccelerationStructure(node) => node.get(graph),
            Self::AccelerationStructureLease(node) => node.get(graph),
        }
    }
}

impl From<AccelerationStructureNode> for AnyAccelerationStructureNode {
    fn from(node: AccelerationStructureNode) -> Self {
        Self::AccelerationStructure(node)
    }
}

impl From<AccelerationStructureLeaseNode> for AnyAccelerationStructureNode {
    fn from(node: AccelerationStructureLeaseNode) -> Self {
        Self::AccelerationStructureLease(node)
    }
}

impl Node for AnyAccelerationStructureNode {
    fn index(self) -> NodeIndex {
        match self {
            Self::AccelerationStructure(node) => node.index(),
            Self::AccelerationStructureLease(node) => node.index(),
        }
    }
}

#[derive(Debug)]
pub enum AnyBufferNode {
    Buffer(BufferNode),
    BufferLease(BufferLeaseNode),
}

impl Clone for AnyBufferNode {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for AnyBufferNode {}

impl Information for AnyBufferNode {
    type Info = BufferInfo;

    fn get(self, graph: &RenderGraph) -> Self::Info {
        match self {
            Self::Buffer(node) => node.get(graph),
            Self::BufferLease(node) => node.get(graph),
        }
    }
}

impl From<BufferNode> for AnyBufferNode {
    fn from(node: BufferNode) -> Self {
        Self::Buffer(node)
    }
}

impl From<BufferLeaseNode> for AnyBufferNode {
    fn from(node: BufferLeaseNode) -> Self {
        Self::BufferLease(node)
    }
}

impl Node for AnyBufferNode {
    fn index(self) -> NodeIndex {
        match self {
            Self::Buffer(node) => node.index(),
            Self::BufferLease(node) => node.index(),
        }
    }
}

#[derive(Debug)]
pub enum AnyImageNode {
    Image(ImageNode),
    ImageLease(ImageLeaseNode),
    SwapchainImage(SwapchainImageNode),
}

impl Clone for AnyImageNode {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for AnyImageNode {}

impl Information for AnyImageNode {
    type Info = ImageInfo;

    fn get(self, graph: &RenderGraph) -> Self::Info {
        match self {
            Self::Image(node) => node.get(graph),
            Self::ImageLease(node) => node.get(graph),
            Self::SwapchainImage(node) => node.get(graph),
        }
    }
}

impl From<ImageNode> for AnyImageNode {
    fn from(node: ImageNode) -> Self {
        Self::Image(node)
    }
}

impl From<ImageLeaseNode> for AnyImageNode {
    fn from(node: ImageLeaseNode) -> Self {
        Self::ImageLease(node)
    }
}

impl From<SwapchainImageNode> for AnyImageNode {
    fn from(node: SwapchainImageNode) -> Self {
        Self::SwapchainImage(node)
    }
}

impl Node for AnyImageNode {
    fn index(self) -> NodeIndex {
        match self {
            Self::Image(node) => node.index(),
            Self::ImageLease(node) => node.index(),
            Self::SwapchainImage(node) => node.index(),
        }
    }
}

pub trait Node: Copy {
    fn index(self) -> NodeIndex;
}

macro_rules! node {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name Node>] {
                pub(super) idx: NodeIndex,
            }

            impl [<$name Node>] {
                pub(super) fn new(idx: NodeIndex) -> Self {
                    Self {
                        idx,
                    }
                }
            }

            impl Clone for [<$name Node>] {
                fn clone(&self) -> Self {
                    *self
                }
            }

            impl Copy for [<$name Node>] {}

            impl Node for [<$name Node>] {
                fn index(self) -> NodeIndex {
                    self.idx
                }
            }
        }
    };
}

node!(AccelerationStructure);
node!(AccelerationStructureLease);
node!(Buffer);
node!(BufferLease);
node!(Image);
node!(ImageLease);
node!(SwapchainImage);

macro_rules! node_unbind {
    ($name:ident) => {
        paste::paste! {
            impl Unbind<RenderGraph, Arc<$name>> for [<$name Node>] {
                fn unbind(self, graph: &mut RenderGraph) -> Arc<$name> {
                    let binding = Arc::clone(
                        graph.bindings[self.idx]
                            .[<as_ $name:snake>]()
                            .unwrap()
                    );
                    graph.bindings[self.idx].unbind();

                    binding
                }
            }
        }
    };
}

node_unbind!(AccelerationStructure);
node_unbind!(Buffer);
node_unbind!(Image);

macro_rules! node_unbind_lease {
    ($name:ident) => {
        paste::paste! {
            impl Unbind<RenderGraph, Arc<Lease<$name>>> for [<$name LeaseNode>] {
                fn unbind(self, graph: &mut RenderGraph) -> Arc<Lease<$name>> {
                    let binding = {
                        let (binding, _) = graph.bindings[self.idx].[<as_ $name:snake _lease_mut>]().unwrap();
                        Arc::clone(binding)
                    };
                    graph.bindings[self.idx].unbind();

                    binding
                }
            }
        }
    };
}

node_unbind_lease!(AccelerationStructure);
node_unbind_lease!(Buffer);
node_unbind_lease!(Image);

pub trait Unbind<Graph, Binding> {
    fn unbind(self, graph: &mut Graph) -> Binding;
}

pub trait View: Node
where
    Self::Information: Clone,
    Self::Subresource: Into<Subresource>,
{
    type Information;
    type Subresource;
}

impl View for AccelerationStructureNode {
    type Information = ();
    type Subresource = ();
}

impl View for AccelerationStructureLeaseNode {
    type Information = ();
    type Subresource = ();
}

impl View for AnyAccelerationStructureNode {
    type Information = ();
    type Subresource = ();
}

impl View for AnyBufferNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for AnyImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for BufferLeaseNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for BufferNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for ImageLeaseNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for ImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for SwapchainImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

#[derive(Debug)]
pub enum ViewType {
    AccelerationStructure,
    Image(ImageViewInfo),
    Buffer(Range<vk::DeviceSize>),
}

impl ViewType {
    pub(super) fn as_buffer(&self) -> Option<&Range<vk::DeviceSize>> {
        match self {
            Self::Buffer(view_info) => Some(view_info),
            _ => None,
        }
    }

    pub(super) fn as_image(&self) -> Option<&ImageViewInfo> {
        match self {
            Self::Image(view_info) => Some(view_info),
            _ => None,
        }
    }
}

// TODO: Remove this
impl From<()> for ViewType {
    fn from(_: ()) -> Self {
        Self::AccelerationStructure
    }
}

impl From<BufferSubresource> for ViewType {
    fn from(subresource: BufferSubresource) -> Self {
        Self::Buffer(subresource.start..subresource.end)
    }
}

impl From<ImageViewInfo> for ViewType {
    fn from(info: ImageViewInfo) -> Self {
        Self::Image(info)
    }
}

impl From<Range<vk::DeviceSize>> for ViewType {
    fn from(range: Range<vk::DeviceSize>) -> Self {
        Self::Buffer(range)
    }
}
