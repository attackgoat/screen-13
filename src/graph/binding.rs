use {
    super::{
        BufferLeaseNode, BufferNode, ImageLeaseNode, ImageNode, RayTraceAccelerationLeaseNode,
        RayTraceAccelerationNode, RenderGraph, Subresource, SubresourceAccess,
        SwapchainImageBinding, SwapchainImageNode,
    },
    crate::{
        driver::{
            Buffer, BufferInfo, DescriptorPool, Image, ImageInfo, RayTraceAcceleration, RenderPass,
            SwapchainImage,
        },
        ptr::Shared,
        Lease,
    },
    archery::SharedPointerKind,
    glam::UVec2,
    log::trace,
    std::{
        fmt::Debug,
        mem::replace,
        ops::{Deref, DerefMut},
    },
    vk_sync::AccessType,
};

#[derive(Debug)]
pub enum AnyBufferBinding<'a, P>
where
    P: SharedPointerKind,
{
    Buffer(&'a mut BufferBinding<P>),
    BufferLeaseBound(&'a mut BufferLeaseBinding<P>),
    BufferLeaseUnbound(&'a mut Lease<BufferBinding<P>, P>),
}

impl<'a, P> From<&'a mut BufferBinding<P>> for AnyBufferBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut BufferBinding<P>) -> Self {
        Self::Buffer(binding)
    }
}

impl<'a, P> From<&'a mut BufferLeaseBinding<P>> for AnyBufferBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut BufferLeaseBinding<P>) -> Self {
        Self::BufferLeaseBound(binding)
    }
}

impl<'a, P> From<&'a mut Lease<BufferBinding<P>, P>> for AnyBufferBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut Lease<BufferBinding<P>, P>) -> Self {
        Self::BufferLeaseUnbound(binding)
    }
}

#[derive(Debug)]
pub enum AnyImageBinding<'a, P>
where
    P: SharedPointerKind,
{
    Image(&'a mut ImageBinding<P>),
    ImageLeaseBound(&'a mut ImageLeaseBinding<P>),
    ImageLeaseUnbound(&'a mut Lease<ImageBinding<P>, P>),
    SwapchainImage(&'a mut SwapchainImageBinding<P>),
}

impl<'a, P> From<&'a mut ImageBinding<P>> for AnyImageBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut ImageBinding<P>) -> Self {
        Self::Image(binding)
    }
}

impl<'a, P> From<&'a mut ImageLeaseBinding<P>> for AnyImageBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut ImageLeaseBinding<P>) -> Self {
        Self::ImageLeaseBound(binding)
    }
}

impl<'a, P> From<&'a mut Lease<ImageBinding<P>, P>> for AnyImageBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut Lease<ImageBinding<P>, P>) -> Self {
        Self::ImageLeaseUnbound(binding)
    }
}

impl<'a, P> From<&'a mut SwapchainImageBinding<P>> for AnyImageBinding<'a, P>
where
    P: SharedPointerKind,
{
    fn from(binding: &'a mut SwapchainImageBinding<P>) -> Self {
        Self::SwapchainImage(binding)
    }
}

pub trait Bind<Graph, Node, P> {
    fn bind(self, graph: Graph) -> Node
    where
        P: SharedPointerKind;
}

#[derive(Debug)]
pub enum Binding<P>
where
    P: SharedPointerKind,
{
    Buffer(BufferBinding<P>, bool),
    BufferLease(BufferLeaseBinding<P>, bool),
    Image(ImageBinding<P>, bool),
    ImageLease(ImageLeaseBinding<P>, bool),
    RayTraceAcceleration(RayTraceAccelerationBinding<P>, bool),
    RayTraceAccelerationLease(RayTraceAccelerationLeaseBinding<P>, bool),
    SwapchainImage(SwapchainImageBinding<P>, bool),
}

impl<P> Binding<P>
where
    P: SharedPointerKind,
{
    pub(super) fn access(&mut self, access: AccessType) -> AccessType {
        match self {
            Self::Buffer(binding, _) => binding.access(access).1,
            Self::BufferLease(binding, _) => binding.access(access).1,
            Self::Image(binding, _) => binding.access(access).1,
            Self::ImageLease(binding, _) => binding.access(access).1,
            Self::RayTraceAcceleration(binding, _) => binding.access(access).1,
            Self::RayTraceAccelerationLease(binding, _) => binding.access(access).1,
            Self::SwapchainImage(binding, _) => binding.access(access).1,
        }
    }

    pub(super) fn as_extent_2d(&self) -> Option<UVec2> {
        Some(match self {
            Self::Image(image, _) => image.item.info.extent_2d(),
            Self::ImageLease(image, _) => image.item.info.extent_2d(),
            Self::SwapchainImage(image, _) => image.item.info.extent_2d(),
            _ => return None,
        })
    }

    pub(super) fn as_image_info(&self) -> Option<ImageInfo> {
        Some(match self {
            Self::Image(binding, _) => binding.item.info,
            Self::ImageLease(binding, _) => binding.item.info,
            Self::SwapchainImage(binding, _) => binding.item.info,
            _ => return None,
        })
    }

    pub(super) fn as_driver_buffer(&self) -> Option<&Buffer<P>> {
        Some(match self {
            Self::Buffer(binding, _) => &binding.item,
            Self::BufferLease(binding, _) => &binding.item,
            _ => return None,
        })
    }

    pub(super) fn as_driver_image(&self) -> Option<&Image<P>> {
        Some(match self {
            Self::Image(binding, _) => &binding.item,
            Self::ImageLease(binding, _) => &binding.item,
            Self::SwapchainImage(binding, _) => &binding.item.image,
            _ => return None,
        })
    }

    pub(super) fn is_bound(&self) -> bool {
        match self {
            Self::Buffer(_, is_bound) => *is_bound,
            Self::BufferLease(_, is_bound) => *is_bound,
            Self::Image(_, is_bound) => *is_bound,
            Self::ImageLease(_, is_bound) => *is_bound,
            Self::RayTraceAcceleration(_, is_bound) => *is_bound,
            Self::RayTraceAccelerationLease(_, is_bound) => *is_bound,
            Self::SwapchainImage(_, is_bound) => *is_bound,
        }
    }

    pub(super) fn unbind(&mut self) {
        *match self {
            Self::Buffer(_, is_bound) => is_bound,
            Self::BufferLease(_, is_bound) => is_bound,
            Self::Image(_, is_bound) => is_bound,
            Self::ImageLease(_, is_bound) => is_bound,
            Self::RayTraceAcceleration(_, is_bound) => is_bound,
            Self::RayTraceAccelerationLease(_, is_bound) => is_bound,
            Self::SwapchainImage(_, is_bound) => is_bound,
        } = false;
    }
}

macro_rules! bind {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name Binding>]<P>
            where
                P: SharedPointerKind,
            {
                pub(super) item: Shared<$name<P>, P>,
                pub(super) access: AccessType,
            }

            impl<P> [<$name Binding>]<P>
            where
                P: SharedPointerKind {
                pub fn new(item: $name<P>) -> Self {
                    let item = Shared::new(item);

                    Self::new_unbind(item, AccessType::Nothing)
                }

                pub(super) fn new_unbind(item: Shared<$name<P>, P>, access: AccessType) -> Self {
                    Self {
                        item,
                        access,
                    }
                }

                /// Allows for direct access to the item inside this binding, without the Shared
                /// wrapper. Returns the previous access type and subresource access which you
                /// should use to create a barrier for whatever access is actually being done.
                pub(super) fn access(&mut self,
                    access: AccessType,
                ) -> (&$name<P>, AccessType) {
                    let previous_access = replace(&mut self.access, access);

                    (&self.item, previous_access)
                }

                /// Allows for direct access to the item inside this binding, without the Shared
                /// wrapper. Returns the previous access type and subresource access which you
                /// should use to create a barrier for whatever access is actually being done.
                pub(super) fn access_mut(&mut self,
                    access: AccessType,
                ) -> (&mut $name<P>, AccessType) {
                    let previous_access = replace(&mut self.access, access);

                    (Shared::get_mut(&mut self.item).unwrap(), previous_access)
                }

                /// Returns a mutable borrow only if no other clones of this shared item exist.
                pub fn get_mut(&mut self) -> Option<&mut $name<P>> {
                    Shared::get_mut(&mut self.item)
                }
            }

            impl<P> Bind<&mut RenderGraph<P>, [<$name Node>]<P>, P> for $name<P>
            where
                P: SharedPointerKind,
            {
                fn bind(self, graph: &mut RenderGraph<P>) -> [<$name Node>]<P> {
                    // We will return a new node
                    let res = [<$name Node>]::new(graph.bindings.len());
                    let binding = Binding::$name([<$name Binding>]::new(self), true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl<P> Bind<&mut RenderGraph<P>, [<$name Node>]<P>, P> for [<$name Binding>]<P>
            where
                P: SharedPointerKind,
            {
                fn bind(self, graph: &mut RenderGraph<P>) -> [<$name Node>]<P> {
                    // We will return a new node
                    // TODO: Maybe return the old node? Tiny bit more efficient in this case
                    let res = [<$name Node>]::new(graph.bindings.len());
                    graph.bindings.push(Binding::$name(self, true));

                    res
                }
            }

            impl<P> Binding<P>
            where
                P: SharedPointerKind,
            {
                pub(super) fn [<as_ $name:snake>](&self) -> &[<$name Binding>]<P> {
                    if let Self::$name(binding, _) = self {
                        &binding
                    } else {
                        panic!("Expected: {} binding", stringify!($name));
                    }
                }
            }
        }
    };
}

bind!(Image);
bind!(Buffer);
bind!(RayTraceAcceleration);

macro_rules! bind_lease {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name LeaseBinding>]<P>(pub Lease<[<$name Binding>]<P>, P>)
            where
                P: SharedPointerKind;

            impl<P> Bind<&mut RenderGraph<P>, [<$name LeaseNode>]<P>, P> for [<$name LeaseBinding>]<P>
            where
                P: SharedPointerKind,
            {
                fn bind(self, graph: &mut RenderGraph<P>) -> [<$name LeaseNode>]<P> {
                    // We will return a new node
                    let res = [<$name LeaseNode>]::new(graph.bindings.len());

                    // We are binding an existing lease binding (ImageLeaseBinding or BufferLeaseBinding or etc)
                    let binding = Binding::[<$name Lease>](self, true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl<P> Bind<&mut RenderGraph<P>, [<$name LeaseNode>]<P>, P> for Lease<[<$name Binding>]<P>, P>
            where
                P: SharedPointerKind,
            {
                fn bind(self, graph: &mut RenderGraph<P>) -> [<$name LeaseNode>]<P> {
                    // We will return a new node
                    let res = [<$name LeaseNode>]::new(graph.bindings.len());

                    // We are binding a new lease (Lease<ImageBinding> or etc)
                    let binding = Binding::[<$name Lease>]([<$name LeaseBinding>](self), true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl<P> Deref for [<$name LeaseBinding>]<P>
            where
                P: SharedPointerKind,
            {
                type Target = [<$name Binding>]<P>;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl<P> DerefMut for [<$name LeaseBinding>]<P>
            where
                P: SharedPointerKind,
            {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            impl<P> Binding<P>
            where
                P: SharedPointerKind,
            {
                pub(super) fn [<as_ $name:snake _lease>](&self) -> &Lease<[<$name Binding>]<P>, P> {
                    if let Self::[<$name Lease>](binding, _) = self {
                        &binding.0
                    } else {
                        panic!("Expected: {} lease binding", stringify!($name));
                    }
                }

                pub(super) fn [<as_ $name:snake _lease_mut>](&mut self) -> &mut Lease<[<$name Binding>]<P>, P> {
                    if let Self::[<$name Lease>](binding, _) = self {
                        &mut binding.0
                    } else {
                        panic!("Expected: {} lease binding", stringify!($name));
                    }
                }
            }
        }
    }
}

bind_lease!(Image);
bind_lease!(Buffer);
bind_lease!(RayTraceAcceleration);

impl<P> BufferBinding<P>
where
    P: SharedPointerKind,
{
    pub fn info(&self) -> &BufferInfo {
        &self.item.info
    }
}

impl<P> ImageBinding<P>
where
    P: SharedPointerKind,
{
    pub fn info(&self) -> &ImageInfo {
        &self.item.info
    }
}

impl<P> ImageLeaseBinding<P>
where
    P: SharedPointerKind,
{
    pub fn info(&self) -> &ImageInfo {
        &self.0.item.info
    }
}

impl<P> SwapchainImageBinding<P>
where
    P: SharedPointerKind,
{
    pub fn info(&self) -> &ImageInfo {
        &self.item.info
    }

    pub fn index(&self) -> usize {
        self.item.idx as usize
    }
}
