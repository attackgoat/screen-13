use {
    super::{
        AccelerationStructureLeaseNode, AccelerationStructureNode, BufferLeaseNode, BufferNode,
        ImageLeaseNode, ImageNode, RenderGraph, SwapchainImageBinding,
    },
    crate::{
        driver::{
            AccelerationStructure, AccelerationStructureInfo, Buffer, BufferInfo, Image, ImageInfo,
        },
        hash_pool::Lease,
    },
    std::{
        fmt::Debug,
        ops::{Deref, DerefMut},
        sync::Arc,
    },
};

#[derive(Debug)]
pub enum AnyBufferBinding<'a> {
    Buffer(&'a mut BufferBinding),
    BufferLeaseBound(&'a mut BufferLeaseBinding),
    BufferLeaseUnbound(&'a mut Lease<BufferBinding>),
}

impl<'a> From<&'a mut BufferBinding> for AnyBufferBinding<'a> {
    fn from(binding: &'a mut BufferBinding) -> Self {
        Self::Buffer(binding)
    }
}

impl<'a> From<&'a mut BufferLeaseBinding> for AnyBufferBinding<'a> {
    fn from(binding: &'a mut BufferLeaseBinding) -> Self {
        Self::BufferLeaseBound(binding)
    }
}

impl<'a> From<&'a mut Lease<BufferBinding>> for AnyBufferBinding<'a> {
    fn from(binding: &'a mut Lease<BufferBinding>) -> Self {
        Self::BufferLeaseUnbound(binding)
    }
}

#[derive(Debug)]
pub enum AnyImageBinding<'a> {
    Image(&'a mut ImageBinding),
    ImageLeaseBound(&'a mut ImageLeaseBinding),
    ImageLeaseUnbound(&'a mut Lease<ImageBinding>),
    SwapchainImage(&'a mut SwapchainImageBinding),
}

impl<'a> From<&'a mut ImageBinding> for AnyImageBinding<'a> {
    fn from(binding: &'a mut ImageBinding) -> Self {
        Self::Image(binding)
    }
}

impl<'a> From<&'a mut ImageLeaseBinding> for AnyImageBinding<'a> {
    fn from(binding: &'a mut ImageLeaseBinding) -> Self {
        Self::ImageLeaseBound(binding)
    }
}

impl<'a> From<&'a mut Lease<ImageBinding>> for AnyImageBinding<'a> {
    fn from(binding: &'a mut Lease<ImageBinding>) -> Self {
        Self::ImageLeaseUnbound(binding)
    }
}

impl<'a> From<&'a mut SwapchainImageBinding> for AnyImageBinding<'a> {
    fn from(binding: &'a mut SwapchainImageBinding) -> Self {
        Self::SwapchainImage(binding)
    }
}

pub trait Bind<Graph, Node> {
    fn bind(self, graph: Graph) -> Node;
}

#[derive(Debug)]
pub enum Binding {
    AccelerationStructure(AccelerationStructureBinding, bool),
    AccelerationStructureLease(AccelerationStructureLeaseBinding, bool),
    Buffer(BufferBinding, bool),
    BufferLease(BufferLeaseBinding, bool),
    Image(ImageBinding, bool),
    ImageLease(ImageLeaseBinding, bool),
    SwapchainImage(SwapchainImageBinding, bool),
}

impl Binding {
    pub(super) fn as_driver_acceleration_structure(&self) -> Option<&AccelerationStructure> {
        Some(match self {
            Self::AccelerationStructure(binding, _) => &binding.item,
            Self::AccelerationStructureLease(binding, _) => &binding.item,
            _ => return None,
        })
    }

    pub(super) fn as_driver_buffer(&self) -> Option<&Buffer> {
        Some(match self {
            Self::Buffer(binding, _) => &binding.item,
            Self::BufferLease(binding, _) => &binding.item,
            _ => return None,
        })
    }

    pub(super) fn as_driver_image(&self) -> Option<&Image> {
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
            Self::AccelerationStructure(_, is_bound) => *is_bound,
            Self::AccelerationStructureLease(_, is_bound) => *is_bound,
            Self::Buffer(_, is_bound) => *is_bound,
            Self::BufferLease(_, is_bound) => *is_bound,
            Self::Image(_, is_bound) => *is_bound,
            Self::ImageLease(_, is_bound) => *is_bound,
            Self::SwapchainImage(_, is_bound) => *is_bound,
        }
    }

    pub(super) fn unbind(&mut self) {
        *match self {
            Self::AccelerationStructure(_, is_bound) => is_bound,
            Self::AccelerationStructureLease(_, is_bound) => is_bound,
            Self::Buffer(_, is_bound) => is_bound,
            Self::BufferLease(_, is_bound) => is_bound,
            Self::Image(_, is_bound) => is_bound,
            Self::ImageLease(_, is_bound) => is_bound,
            Self::SwapchainImage(_, is_bound) => is_bound,
        } = false;
    }
}

macro_rules! bind {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name Binding>] {
                pub(super) item: Arc<$name>,
            }

            impl [<$name Binding>] {
                pub fn new(item: $name) -> Self {
                    let item = Arc::new(item);

                    Self {
                        item,
                    }
                }

                /// Returns a borrow.
                pub fn get(&self) -> &$name {
                    &self.item
                }

                /// Returns a mutable borrow only if no other clones of this shared item exist.
                pub fn get_mut(&mut self) -> Option<&mut $name> {
                    Arc::get_mut(&mut self.item)
                }
            }

            impl Bind<&mut RenderGraph, [<$name Node>]> for $name {
                fn bind(self, graph: &mut RenderGraph) -> [<$name Node>] {
                    // In this function we are binding a new item (Image or Buffer or etc)

                    // We will return a new node
                    let res = [<$name Node>]::new(graph.bindings.len());
                    let binding = Binding::$name([<$name Binding>]::new(self), true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl Bind<&mut RenderGraph, [<$name Node>]> for [<$name Binding>] {
                fn bind(self, graph: &mut RenderGraph) -> [<$name Node>] {
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

            impl Binding {
                pub(super) fn [<as_ $name:snake>](&self) -> Option<&[<$name Binding>]> {
                    if let Self::$name(binding, _) = self {
                        Some(&binding)
                    } else {
                        None
                    }
                }

                pub(super) fn [<as_ $name:snake _mut>](&mut self) -> Option<(&mut [<$name Binding>], &mut bool)> {
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

bind!(AccelerationStructure);
bind!(Image);
bind!(Buffer);

macro_rules! bind_lease {
    ($name:ident) => {
        paste::paste! {
            #[derive(Debug)]
            pub struct [<$name LeaseBinding>](pub Lease<[<$name Binding>]>);

            impl Bind<&mut RenderGraph, [<$name LeaseNode>]> for [<$name LeaseBinding>] {
                fn bind(self, graph: &mut RenderGraph) -> [<$name LeaseNode>] {
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

            impl Bind<&mut RenderGraph, [<$name LeaseNode>]> for Lease<[<$name Binding>]> {
                fn bind(self, graph: &mut RenderGraph) -> [<$name LeaseNode>] {
                    // In this function we are binding a new lease (Lease<ImageBinding> or etc)

                    // We will return a new node
                    let res = [<$name LeaseNode>]::new(graph.bindings.len());
                    let binding = Binding::[<$name Lease>]([<$name LeaseBinding>](self), true);
                    graph.bindings.push(binding);

                    res
                }
            }

            impl Deref for [<$name LeaseBinding>] {
                type Target = [<$name Binding>];

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl DerefMut for [<$name LeaseBinding>] {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            impl Binding {
                pub(super) fn [<as_ $name:snake _lease>](&self) -> Option<&Lease<[<$name Binding>]>> {
                    if let Self::[<$name Lease>](binding, _) = self {
                        Some(&binding.0)
                    } else {
                        None
                    }
                }

                pub(super) fn [<as_ $name:snake _lease_mut>](&mut self) -> Option<(&mut Lease<[<$name Binding>]>, &mut bool)> {
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

bind_lease!(AccelerationStructure);
bind_lease!(Image);
bind_lease!(Buffer);

impl AccelerationStructureBinding {
    pub fn info(&self) -> &AccelerationStructureInfo {
        &self.item.info
    }
}

impl BufferLeaseBinding {
    pub fn info(&self) -> &BufferInfo {
        &self.item.info
    }
}

impl BufferBinding {
    pub fn info(&self) -> &BufferInfo {
        &self.item.info
    }
}

impl ImageBinding {
    pub fn info(&self) -> &ImageInfo {
        &self.item.info
    }
}

impl ImageLeaseBinding {
    pub fn info(&self) -> &ImageInfo {
        &self.0.item.info
    }
}

impl SwapchainImageBinding {
    pub fn info(&self) -> &ImageInfo {
        &self.item.info
    }

    pub fn index(&self) -> usize {
        self.item.idx as usize
    }
}
