use {
    super::{
        BufferLeaseNode, BufferNode, ImageLeaseNode, ImageNode, RenderGraph, SwapchainImageNode,
    },
    crate::driver::{BufferInfo, ImageInfo},
    archery::SharedPointerKind,
};

pub trait Information {
    type Info;

    fn get(self, graph: &RenderGraph<impl SharedPointerKind>) -> Self::Info;
}

macro_rules! information {
    ($name:ident: $src:ident -> $dst:ident) => {
        paste::paste! {
            impl<P> Information for $src<P> {
                type Info = $dst;

                fn get(self, graph: &RenderGraph<impl SharedPointerKind>) -> $dst {
                    graph.bindings[self.idx].[<as_ $name>]().info().clone()
                }
            }
        }
    };
}

information!(buffer: BufferNode -> BufferInfo);
information!(buffer_lease: BufferLeaseNode -> BufferInfo);
information!(image: ImageNode -> ImageInfo);
information!(image_lease: ImageLeaseNode -> ImageInfo);
information!(swapchain_image: SwapchainImageNode -> ImageInfo);
