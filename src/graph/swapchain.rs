use {
    super::{Bind, Binding, RenderGraph, Resolver, SwapchainImageNode, Unbind},
    crate::driver::swapchain::SwapchainImage,
};

impl Bind<&mut RenderGraph, SwapchainImageNode> for SwapchainImage {
    fn bind(self, graph: &mut RenderGraph) -> SwapchainImageNode {
        // We will return a new node
        let res = SwapchainImageNode::new(graph.bindings.len());

        //trace!("Node {}: {:?}", res.idx, &self);

        let binding = Binding::SwapchainImage(Box::new(self), true);
        graph.bindings.push(binding);

        res
    }
}

impl Binding {
    pub(super) fn as_swapchain_image(&self) -> Option<&SwapchainImage> {
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
            .clone()
    }
}
