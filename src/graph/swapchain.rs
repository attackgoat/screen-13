use {
    super::{Bind, Binding, RenderGraph, Resolver, Subresource, SwapchainImageNode, Unbind},
    crate::{
        driver::{Image, SwapchainImage},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    std::{fmt::Debug, mem::replace},
    vk_sync::AccessType,
};

#[derive(Debug)]
pub struct SwapchainImageBinding<P>
where
    P: SharedPointerKind,
{
    pub(super) item: SwapchainImage<P>,
    pub(super) previous_access: AccessType,
    pub(super) previous_subresource: Option<Subresource>,
}

impl<P> SwapchainImageBinding<P>
where
    P: SharedPointerKind,
{
    pub(super) fn new(item: SwapchainImage<P>) -> Self {
        Self::new_unbind(item, AccessType::Nothing, None)
    }

    pub(super) fn new_unbind(
        item: SwapchainImage<P>,
        previous_access: AccessType,
        previous_subresource: Option<Subresource>,
    ) -> Self {
        Self {
            item,
            previous_access,
            previous_subresource,
        }
    }

    pub(super) fn next_access(
        &mut self,
        access: AccessType,
        subresource: Option<Subresource>,
    ) -> (AccessType, Option<Subresource>) {
        (
            replace(&mut self.previous_access, access),
            replace(&mut self.previous_subresource, subresource),
        )
    }

    /// Allows for direct access to the item inside this binding, without the Shared
    /// wrapper. Returns the previous access type and subresource access which you
    /// should use to create a barrier for whatever access is actually being done.
    pub fn access_inner(
        &mut self,
        access: AccessType,
    ) -> (&Image<P>, AccessType, Option<Subresource>) {
        self.access_inner_subresource(access, None)
    }

    /// Allows for direct access to the item inside this binding, without the Shared
    /// wrapper. Returns the previous access type and subresource access which you
    /// should use to create a barrier for whatever access is actually being done.
    pub fn access_inner_subresource(
        &mut self,
        access: AccessType,
        subresource: impl Into<Option<Subresource>>,
    ) -> (&Image<P>, AccessType, Option<Subresource>) {
        let subresource = subresource.into();
        let (previous_access, previous_subresource) = self.next_access(access, subresource);

        (&self.item, previous_access, previous_subresource)
    }
}

impl<P> Bind<&mut RenderGraph<P>, SwapchainImageNode<P>, P> for SwapchainImage<P>
where
    P: SharedPointerKind,
{
    fn bind(self, graph: &mut RenderGraph<P>) -> SwapchainImageNode<P> {
        // We will return a new node
        let res = SwapchainImageNode::new(graph.bindings.len());

        //trace!("Node {}: {:?}", res.idx, &self);

        let binding = Binding::SwapchainImage(SwapchainImageBinding::new(self), true);
        graph.bindings.push(binding);

        res
    }
}

impl<P> Bind<&mut RenderGraph<P>, SwapchainImageNode<P>, P> for SwapchainImageBinding<P>
where
    P: SharedPointerKind,
{
    fn bind(self, graph: &mut RenderGraph<P>) -> SwapchainImageNode<P> {
        // We will return a new node
        let res = SwapchainImageNode::new(graph.bindings.len());

        //trace!("Node {}: {:?}", res.idx, &self);

        graph.bindings.push(Binding::SwapchainImage(self, true));

        res
    }
}

impl<P> Binding<P>
where
    P: SharedPointerKind,
{
    pub(super) fn as_swapchain_image(&self) -> &SwapchainImageBinding<P> {
        if let Self::SwapchainImage(binding, true) = self {
            binding
        } else if let Self::SwapchainImage(_, false) = self {
            // User code might try this - but it is a programmer error
            // to access a binding after it has been unbound so dont
            panic!();
        } else {
            // The private code in this module should prevent this branch
            unreachable!("boom");
        }
    }
}

impl<P> Unbind<Resolver<P>, SwapchainImage<P>> for SwapchainImageNode<P>
where
    P: SharedPointerKind,
{
    // We allow the resolver to unbind a swapchain node directly into a shared image
    fn unbind(self, graph: &mut Resolver<P>) -> SwapchainImage<P> {
        graph.graph.bindings[self.idx]
            .as_swapchain_image()
            .item
            .clone()
    }
}