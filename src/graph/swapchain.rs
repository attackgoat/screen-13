use {
    super::{Bind, Binding, RenderGraph, Resolver, SwapchainImageNode, Unbind},
    crate::driver::SwapchainImage,
    std::{fmt::Debug, mem::replace},
    vk_sync::AccessType,
};

#[derive(Debug)]
pub struct SwapchainImageBinding {
    pub(super) item: SwapchainImage,
    pub(super) access: AccessType,
}

impl SwapchainImageBinding {
    pub(super) fn new(item: SwapchainImage) -> Self {
        Self::new_unbind(item, AccessType::Nothing)
    }

    pub(super) fn new_unbind(item: SwapchainImage, access: AccessType) -> Self {
        Self { item, access }
    }

    /// Returns the previous access type and subresource access which you should use to create a
    /// barrier for whatever access is actually being done.
    pub(super) fn access_mut(&mut self, access: AccessType) -> AccessType {
        replace(&mut self.access, access)
    }
}

impl Bind<&mut RenderGraph, SwapchainImageNode> for SwapchainImage {
    fn bind(self, graph: &mut RenderGraph) -> SwapchainImageNode {
        // We will return a new node
        let res = SwapchainImageNode::new(graph.bindings.len());

        //trace!("Node {}: {:?}", res.idx, &self);

        let binding = Binding::SwapchainImage(SwapchainImageBinding::new(self), true);
        graph.bindings.push(binding);

        res
    }
}

impl Bind<&mut RenderGraph, SwapchainImageNode> for SwapchainImageBinding {
    fn bind(self, graph: &mut RenderGraph) -> SwapchainImageNode {
        // We will return a new node
        let res = SwapchainImageNode::new(graph.bindings.len());

        graph.bindings.push(Binding::SwapchainImage(self, true));

        res
    }
}

impl Binding {
    pub(super) fn as_swapchain_image(&self) -> Option<&SwapchainImageBinding> {
        if let Self::SwapchainImage(binding, true) = self {
            Some(binding)
        } else if let Self::SwapchainImage(_, false) = self {
            // User code might try this - but it is a programmer error
            // to access a binding after it has been unbound so dont
            None
        } else {
            // The private code in this module should prevent this branch
            unreachable!();
        }
    }
}

impl Unbind<Resolver, SwapchainImage> for SwapchainImageNode {
    // We allow the resolver to unbind a swapchain node directly into a shared image
    fn unbind(self, graph: &mut Resolver) -> SwapchainImage {
        graph.graph.bindings[self.idx]
            .as_swapchain_image()
            .unwrap()
            .item
            .clone()
    }
}
