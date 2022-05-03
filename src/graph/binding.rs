use {
    super::{
        BufferLeaseNode, BufferNode, ImageLeaseNode, ImageNode, RayTraceAccelerationLeaseNode,
        RayTraceAccelerationNode, RenderGraph, SwapchainImageBinding,
    },
    crate::{
        driver::{Buffer, BufferInfo, Image, ImageInfo, RayTraceAcceleration},
        Lease,
    },
    archery::{SharedPointer, SharedPointerKind},
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
    pub(super) fn access_mut(&mut self, access: AccessType) -> AccessType {
        match self {
            Self::Buffer(binding, _) => binding.access_mut(access),
            Self::BufferLease(binding, _) => binding.access_mut(access),
            Self::Image(binding, _) => binding.access_mut(access),
            Self::ImageLease(binding, _) => binding.access_mut(access),
            Self::RayTraceAcceleration(binding, _) => binding.access_mut(access),
            Self::RayTraceAccelerationLease(binding, _) => binding.access_mut(access),
            Self::SwapchainImage(binding, _) => binding.access_mut(access),
        }
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

    pub(super) fn image_info(&self) -> Option<ImageInfo> {
        Some(match self {
            Self::Image(binding, _) => binding.item.info,
            Self::ImageLease(binding, _) => binding.item.info,
            Self::SwapchainImage(binding, _) => binding.item.info,
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
                pub(super) item: SharedPointer<$name<P>, P>,
                pub(super) access: AccessType,
            }

            impl<P> [<$name Binding>]<P>
            where
                P: SharedPointerKind {
                pub fn new(item: $name<P>) -> Self {
                    let item = SharedPointer::new(item);

                    Self::new_unbind(item, AccessType::Nothing)
                }

                pub(super) fn new_unbind(item: SharedPointer<$name<P>, P>, access: AccessType) -> Self {
                    Self {
                        item,
                        access,
                    }
                }

                /// Returns the previous access type and subresource access which you should use to
                /// create a barrier for whatever access is actually being done.
                pub(super) fn access_mut(&mut self,
                    access: AccessType,
                ) -> AccessType {
                    replace(&mut self.access, access)
                }

                /// Returns a mutable borrow only if no other clones of this shared item exist.
                pub fn get_mut(&mut self) -> Option<&mut $name<P>> {
                    SharedPointer::get_mut(&mut self.item)
                }
            }

            impl<P> Bind<&mut RenderGraph<P>, [<$name Node>]<P>, P> for $name<P>
            where
                P: SharedPointerKind,
            {
                fn bind(self, graph: &mut RenderGraph<P>) -> [<$name Node>]<P> {
                    // In this function we are binding a new item (Image or Buffer or etc)

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
                    // In this function we are binding an existing binding (ImageBinding or
                    // BufferBinding or etc)

                    // We will return an existing node, if possible
                    // TODO: Could store a sorted list of these shared pointers to avoid the O(N)
                    let item = **self.item;
                    for (idx, existing_binding) in graph.bindings.iter_mut().enumerate() {
                        if let Some((existing_binding, is_bound)) = existing_binding.[<as_ $name:snake _mut>]() {
                            if **existing_binding.item == item {
                                *is_bound = true;

                                return [<$name Node>]::new(idx);
                            }
                        }
                    }

                    // Return a new node
                    let res = [<$name Node>]::new(graph.bindings.len());
                    let binding = Binding::$name(self, true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl<P> Binding<P>
            where
                P: SharedPointerKind,
            {
                pub(super) fn [<as_ $name:snake>](&self) -> Option<&[<$name Binding>]<P>> {
                    if let Self::$name(binding, _) = self {
                        Some(&binding)
                    } else {
                        None
                    }
                }

                pub(super) fn [<as_ $name:snake _mut>](&mut self) -> Option<(&mut [<$name Binding>]<P>, &mut bool)> {
                    if let Self::$name(ref mut binding, ref mut is_bound) = self {
                        Some((binding, is_bound))
                    } else {
                        None
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
                    // In this function we are binding an existing lease binding
                    // (ImageLeaseBinding or BufferLeaseBinding or etc)

                    // We will return an existing node, if possible
                    // TODO: Could store a sorted list of these shared pointers to avoid the O(N)
                    let item = **self.item;
                    for (idx, existing_binding) in graph.bindings.iter_mut().enumerate() {
                        if let Some((existing_binding, is_bound)) = existing_binding.[<as_ $name:snake _lease_mut>]() {
                            if **existing_binding.item == item {
                                *is_bound = true;

                                return [<$name LeaseNode>]::new(idx);
                            }
                        }
                    }

                    // We will return a new node
                    let res = [<$name LeaseNode>]::new(graph.bindings.len());
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
                    // In this function we are binding a new lease (Lease<ImageBinding> or etc)

                    // We will return a new node
                    let res = [<$name LeaseNode>]::new(graph.bindings.len());
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
                // TODO: Remove lint after ray tracing baked in
                #[allow(dead_code)]
                pub(super) fn [<as_ $name:snake _lease>](&self) -> Option<&Lease<[<$name Binding>]<P>, P>> {
                    if let Self::[<$name Lease>](binding, _) = self {
                        Some(&binding.0)
                    } else {
                        None
                    }
                }

                // TODO: Remove lint after ray tracing baked in
                #[allow(dead_code)]
                pub(super) fn [<as_ $name:snake _lease_mut>](&mut self) -> Option<(&mut Lease<[<$name Binding>]<P>, P>, &mut bool)> {
                    if let Self::[<$name Lease>](ref mut binding, ref mut is_bound) = self {
                        Some((&mut binding.0, is_bound))
                    } else {
                        None
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
