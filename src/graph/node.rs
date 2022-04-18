use {
    super::{
        BufferBinding, BufferLeaseBinding, ImageBinding, ImageLeaseBinding, Information, NodeIndex,
        RayTraceAccelerationBinding, RayTraceAccelerationLeaseBinding, RenderGraph, Subresource,
    },
    crate::{
        driver::{vk, BufferInfo, BufferSubresource, ImageInfo, ImageSubresource, ImageViewInfo},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    std::{marker::PhantomData, ops::Range},
};

#[derive(Debug)]
pub enum AnyBufferNode<P> {
    Buffer(BufferNode<P>),
    BufferLease(BufferLeaseNode<P>),
}

impl<P> Clone for AnyBufferNode<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for AnyBufferNode<P> {}

impl<P> Information for AnyBufferNode<P> {
    type Info = BufferInfo;

    fn get(self, graph: &RenderGraph<impl SharedPointerKind + Send>) -> Self::Info {
        match self {
            Self::Buffer(node) => node.get(graph),
            Self::BufferLease(node) => node.get(graph),
        }
    }
}

impl<P> From<BufferNode<P>> for AnyBufferNode<P> {
    fn from(node: BufferNode<P>) -> Self {
        Self::Buffer(node)
    }
}

impl<P> From<BufferLeaseNode<P>> for AnyBufferNode<P> {
    fn from(node: BufferLeaseNode<P>) -> Self {
        Self::BufferLease(node)
    }
}

impl<P> Node<P> for AnyBufferNode<P> {
    fn index(self) -> NodeIndex {
        match self {
            Self::Buffer(node) => node.index(),
            Self::BufferLease(node) => node.index(),
        }
    }
}

#[derive(Debug)]
pub enum AnyImageNode<P> {
    Image(ImageNode<P>),
    ImageLease(ImageLeaseNode<P>),
    SwapchainImage(SwapchainImageNode<P>),
}

impl<P> Clone for AnyImageNode<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for AnyImageNode<P> {}

impl<P> Information for AnyImageNode<P> {
    type Info = ImageInfo;

    fn get(self, graph: &RenderGraph<impl SharedPointerKind + Send>) -> Self::Info {
        match self {
            Self::Image(node) => node.get(graph),
            Self::ImageLease(node) => node.get(graph),
            Self::SwapchainImage(node) => node.get(graph),
        }
    }
}

impl<P> From<ImageNode<P>> for AnyImageNode<P> {
    fn from(node: ImageNode<P>) -> Self {
        Self::Image(node)
    }
}

impl<P> From<ImageLeaseNode<P>> for AnyImageNode<P> {
    fn from(node: ImageLeaseNode<P>) -> Self {
        Self::ImageLease(node)
    }
}

impl<P> From<SwapchainImageNode<P>> for AnyImageNode<P> {
    fn from(node: SwapchainImageNode<P>) -> Self {
        Self::SwapchainImage(node)
    }
}

impl<P> Node<P> for AnyImageNode<P> {
    fn index(self) -> NodeIndex {
        match self {
            Self::Image(node) => node.index(),
            Self::ImageLease(node) => node.index(),
            Self::SwapchainImage(node) => node.index(),
        }
    }
}

pub trait Node<P>: Copy {
    fn index(self) -> NodeIndex;
}

macro_rules! node {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name Node>]<P> {
                __: PhantomData<P>,
                pub(super) idx: NodeIndex,
            }

            impl<P> [<$name Node>]<P> {
                pub(super) fn new(idx: NodeIndex) -> Self {
                    Self {
                        __: PhantomData,
                        idx,
                    }
                }
            }

            impl<P> Clone for [<$name Node>]<P> {
                fn clone(&self) -> Self {
                    *self
                }
            }

            impl<P> Copy for [<$name Node>]<P> {}

            impl<P> Node<P> for [<$name Node>]<P>  {
                fn index(self) -> NodeIndex {
                    self.idx
                }
            }
        }
    };
}

node!(Buffer);
node!(BufferLease);
node!(Image);
node!(ImageLease);
node!(RayTraceAcceleration);
node!(RayTraceAccelerationLease);
node!(SwapchainImage);

macro_rules! node_unbind {
    ($name:ident) => {
        paste::paste! {
            impl<P> Unbind<RenderGraph<P>, [<$name Binding>]<P>> for [<$name Node>]<P>
            where
                P: SharedPointerKind + Send + 'static,
            {
                fn unbind(self, graph: &mut RenderGraph<P>) -> [<$name Binding>]<P> {
                    let binding = {
                        let binding = graph.bindings[self.idx].[<as_ $name:snake>]().unwrap();
                        let item = Shared::clone(&binding.item);

                        // When unbinding we return a binding that has the last access type set to
                        // whatever the last acccess in the graph was (because it will be valid once
                        // the graph is resolved and you should not use an unbound binding before
                        // the graph is resolved. Resolve it and then use said binding on a
                        // different graph.)
                        let previous_access = graph.last_access(self)
                            .unwrap_or(binding.access).clone();
                        [<$name Binding>]::new_unbind(item, previous_access)
                    };
                    graph.bindings[self.idx].unbind();

                    binding
                }
            }
        }
    };
}

node_unbind!(Buffer);
node_unbind!(Image);
node_unbind!(RayTraceAcceleration);

macro_rules! node_unbind_lease {
    ($name:ident) => {
        paste::paste! {
            impl<P> Unbind<RenderGraph<P>, [<$name LeaseBinding>]<P>> for [<$name LeaseNode>]<P>
            where
                P: SharedPointerKind + Send + 'static,
            {
                fn unbind(self, graph: &mut RenderGraph<P>) -> [<$name LeaseBinding>]<P> {
                    let binding = {
                        let last_access = graph.last_access(self);
                        let (binding, _) = graph.bindings[self.idx].[<as_ $name:snake _lease_mut>]().unwrap();
                        let item = binding.item.clone();

                        // When unbinding we return a binding that has the last access type set to
                        // whatever the last acccess in the graph was (because it will be valid once
                        // the graph is resolved and you should not use an unbound binding before
                        // the graph is resolved. Resolve it and then use said binding on a
                        // different graph.)
                        let previous_access = last_access.unwrap_or(binding.access);
                        let item_binding = [<$name Binding>]::new_unbind(
                            item,
                            previous_access,
                        );

                        // Move the return-to-pool-on-drop behavior to a new lease
                        let lease = binding.transfer(item_binding);

                        [<$name LeaseBinding>](lease)
                    };
                    graph.bindings[self.idx].unbind();

                    binding
                }
            }
        }
    };
}

node_unbind_lease!(Buffer);
node_unbind_lease!(Image);
node_unbind_lease!(RayTraceAcceleration);

pub trait Unbind<Graph, Binding> {
    fn unbind(self, graph: &mut Graph) -> Binding;
}

pub trait View<P>: Node<P>
where
    Self::Information: Clone,
    Self::Subresource: Into<Subresource>,
{
    type Information;
    type Subresource;
}

impl<P> View<P> for AnyBufferNode<P> {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl<P> View<P> for AnyImageNode<P> {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl<P> View<P> for BufferLeaseNode<P> {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl<P> View<P> for BufferNode<P> {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl<P> View<P> for ImageLeaseNode<P> {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl<P> View<P> for ImageNode<P> {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl<P> View<P> for SwapchainImageNode<P> {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

pub enum ViewType {
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
