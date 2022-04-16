use {
    super::{
        AnyBufferNode, AnyImageNode, Attachment, AttachmentIndex, Bind, Binding, BufferLeaseNode,
        BufferNode, Descriptor, Edge, Execution, ExecutionFunction, ExecutionPipeline,
        ImageLeaseNode, ImageNode, Information, Node, NodeIndex, Pass, PushConstantRange,
        RayTraceAccelerationNode, Rect, RenderGraph, SampleCount, Subresource, SubresourceAccess,
        SwapchainImageNode, View, ViewType,
    },
    crate::{
        driver::{
            Buffer, ComputePipeline, DepthStencilMode, GraphicPipeline, Image, ImageViewInfo,
            RayTraceAcceleration, RayTracePipeline,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    ash::vk,
    glam::{ivec2, uvec2, vec2, UVec3},
    log::warn,
    meshopt::any_as_u8_slice,
    std::{
        marker::PhantomData,
        mem::take,
        ops::{Index, Range},
    },
    vk_sync::AccessType,
};

unsafe fn push_constants(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    push_consts: impl IntoIterator<Item = PushConstantRange>,
    layout: vk::PipelineLayout,
) {
    for push_const in push_consts.into_iter() {
        device.cmd_push_constants(
            cmd_buf,
            layout,
            push_const.stage,
            push_const.offset,
            push_const.data.as_slice(),
        );
    }
}

pub trait Access {
    const DEFAULT_READ: AccessType;
    const DEFAULT_WRITE: AccessType;
}

impl<P> Access for ComputePipeline<P>
where
    P: SharedPointerKind,
{
    const DEFAULT_READ: AccessType = AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::ComputeShaderWrite;
}

impl<P> Access for GraphicPipeline<P>
where
    P: SharedPointerKind,
{
    const DEFAULT_READ: AccessType = AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::AnyShaderWrite;
}

impl<P> Access for RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    const DEFAULT_READ: AccessType =
        AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::Nothing;
}

macro_rules! bind {
    ($name:ident) => {
        paste::paste! {
            impl<'a, P> Bind<PassRef<'a, P>, PipelinePassRef<'a, [<$name Pipeline>]<P>, P>, P> for &'a Shared<[<$name Pipeline>]<P>, P>
            where
                P: SharedPointerKind + Send + 'static,
            {
                // TODO: Allow binding as explicit secondary command buffers? like with compute/raytrace stuff
                fn bind(self, mut pass: PassRef<'a, P>) -> PipelinePassRef<'_, [<$name Pipeline>]<P>, P> {
                    let pass_ref = pass.as_mut();
                    if pass_ref.execs.last().unwrap().pipeline.is_some() {
                        // Binding from PipelinePass -> PipelinePass (changing shaders)
                        pass_ref.execs.push(Default::default());
                    }

                    pass_ref.execs.last_mut().unwrap().pipeline = Some(ExecutionPipeline::$name(Shared::clone(self)));

                    PipelinePassRef {
                        __: PhantomData,
                        pass,
                    }
                }
            }

            impl<P> ExecutionPipeline<P>
            where
                P: SharedPointerKind,
            {
                #[allow(unused)]
                pub(super) fn [<is_ $name:snake>](&self) -> bool {
                    matches!(self, Self::$name(_))
                }

                #[allow(unused)]
                pub(super) fn [<unwrap_ $name:snake>](&self) -> &Shared<[<$name Pipeline>]<P>, P> {
                    if let Self::$name(binding) = self {
                        &binding
                    } else {
                        panic!();
                    }
                }
            }
        }
    };
}

// Pipelines you can bind to a pass
bind!(Compute);
bind!(Graphic);
bind!(RayTrace);

pub struct Bindings<'a, P>
where
    P: SharedPointerKind,
{
    pub(super) exec: &'a Execution<P>,
    pub(super) graph: &'a RenderGraph<P>,
}

impl<'a, P> Bindings<'a, P>
where
    P: SharedPointerKind,
{
    fn binding_ref(&self, node_idx: usize) -> &Binding<P> {
        // You must have called read or write for this node on this execution before indexing
        // into the bindings data!
        debug_assert!(
            self.exec.accesses.contains_key(&node_idx),
            "Expected call to read or write before indexing the bindings"
        );

        &self.graph.bindings[node_idx]
    }
}

macro_rules! index {
    ($name:ident, $handle:ident) => {
        paste::paste! {
            impl<'a, P> Index<[<$name Node>]<P>> for Bindings<'a, P>
            where
                P: SharedPointerKind,
            {
                type Output = $handle<P>;

                fn index(&self, node: [<$name Node>]<P>) -> &Self::Output {
                    &*self.binding_ref(node.idx).[<as_ $name:snake>]().unwrap().item
                }
            }
        }
    };
}

// Allow indexing the Bindings data during command execution:
// (This gets you access to the driver images or other resources)
index!(Buffer, Buffer);
index!(BufferLease, Buffer);
index!(Image, Image);
index!(ImageLease, Image);
index!(RayTraceAcceleration, RayTraceAcceleration);
index!(SwapchainImage, Image);

impl<'a, P> Index<AnyImageNode<P>> for Bindings<'a, P>
where
    P: SharedPointerKind,
{
    type Output = Image<P>;

    fn index(&self, node: AnyImageNode<P>) -> &Self::Output {
        let node_idx = match node {
            AnyImageNode::Image(node) => node.idx,
            AnyImageNode::ImageLease(node) => node.idx,
            AnyImageNode::SwapchainImage(node) => node.idx,
        };
        let binding = self.binding_ref(node_idx);

        match node {
            AnyImageNode::Image(_) => &binding.as_image().unwrap().item,
            AnyImageNode::ImageLease(_) => &binding.as_image_lease().unwrap().item,
            AnyImageNode::SwapchainImage(_) => &binding.as_swapchain_image().unwrap().item,
        }
    }
}

impl<'a, P> Index<AnyBufferNode<P>> for Bindings<'a, P>
where
    P: SharedPointerKind,
{
    type Output = Buffer<P>;

    fn index(&self, node: AnyBufferNode<P>) -> &Self::Output {
        let node_idx = match node {
            AnyBufferNode::Buffer(node) => node.idx,
            AnyBufferNode::BufferLease(node) => node.idx,
        };
        let binding = self.binding_ref(node_idx);

        match node {
            AnyBufferNode::Buffer(_) => &binding.as_buffer().unwrap().item,
            AnyBufferNode::BufferLease(_) => &binding.as_buffer_lease().unwrap().item,
        }
    }
}

pub struct PassRef<'a, P>
where
    P: SharedPointerKind + Send,
{
    pub(super) exec_idx: usize,
    pub(super) graph: &'a mut RenderGraph<P>,
    pub(super) pass_idx: usize,
}

impl<'a, P> PassRef<'a, P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub(super) fn new(graph: &'a mut RenderGraph<P>, name: String) -> PassRef<'a, P> {
        let pass_idx = graph.passes.len();
        graph.passes.push(Pass {
            load_attachments: Default::default(),
            resolve_attachments: Default::default(),
            store_attachments: Default::default(),
            depth_stencil: None,
            execs: vec![Default::default()], // We start off with a default execution!
            name,
            push_consts: vec![],
            render_area: None,
            scissor: None,
            subpasses: vec![],
            viewport: None,
        });

        Self {
            exec_idx: 0,
            graph,
            pass_idx,
        }
    }

    /// Instruct the graph to provide vulkan barriers around access to the given node.
    ///
    /// The resolver will insert an execution barrier into the command buffer as required using
    /// the provided access type. This allows you to do whatever was declared here inside an
    /// excution callback registered after this call.
    pub fn access_node(mut self, node: impl Node<P> + Information, access: AccessType) -> Self {
        self.assert_bound_graph_node(node);

        let idx = node.index();
        let binding = &self.graph.bindings[idx];

        let mut node_access_range = None;
        if let Some(buf) = binding.as_driver_buffer() {
            node_access_range = Some(Subresource::Buffer((0..buf.info.size).into()));
        } else if let Some(image) = binding.as_driver_image() {
            node_access_range = Some(Subresource::Image(image.info.default_view_info().into()))
        }

        self.push_node_access(node, access, node_access_range);
        self
    }

    /// Instruct the graph to provide vulkan barriers around access to the given node with
    /// specific information about the subresource being accessed.
    ///
    /// The resolver will insert an execution barrier into the command buffer as required using
    /// the provided access type. This allows you to do whatever was declared here inside an
    /// excution callback registered after this call.
    pub fn access_node_subrange<N>(
        mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View<P>,
    {
        self.push_node_access(node, access, Some(subresource.into().into()));
        self
    }

    fn as_mut(&mut self) -> &mut Pass<P> {
        &mut self.graph.passes[self.pass_idx]
    }

    fn as_ref(&self) -> &Pass<P> {
        &self.graph.passes[self.pass_idx]
    }

    fn assert_bound_graph_node<N>(&self, node: impl Node<N>) {
        let idx = node.index();

        assert!(self.graph.bindings[idx].is_bound());
    }

    pub fn bind_pipeline<B>(self, binding: B) -> <B as Edge<Self>>::Result
    where
        B: Edge<Self>,
        B: Bind<Self, <B as Edge<Self>>::Result, P>,
    {
        binding.bind(self)
    }

    pub fn execute(
        mut self,
        func: impl FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send + 'static,
    ) -> Self {
        self.push_execute(func);
        self
    }

    fn push_execute(
        &mut self,
        func: impl FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send + 'static,
    ) {
        let pass = self.as_mut();
        let exec = {
            let last_exec = pass.execs.last_mut().unwrap();
            last_exec.func = Some(ExecutionFunction(Box::new(func)));

            let mut next_exec = Execution::default();
            next_exec.pipeline = last_exec.pipeline.clone();
            next_exec
        };

        pass.execs.push(exec);
        self.exec_idx += 1;
    }

    fn push_node_access(
        &mut self,
        node: impl Node<P>,
        access: AccessType,
        subresource: Option<Subresource>,
    ) {
        let node_idx = node.index();
        self.assert_bound_graph_node(node);

        let access = SubresourceAccess {
            access,
            subresource,
        };
        self.as_mut()
            .execs
            .last_mut()
            .unwrap()
            .accesses
            .entry(node_idx)
            .and_modify(|accesses| accesses[1] = access)
            .or_insert([access, access]);
    }

    pub fn read_node(self, node: impl Node<P> + Information) -> Self {
        let access = AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer;
        self.access_node(node, access)
    }

    pub fn submit_pass(self) -> &'a mut RenderGraph<P> {
        // If nothing was done in this pass we can just ignore it
        if self.exec_idx == 0 {
            self.graph.passes.pop();
        }

        self.graph
    }

    pub fn write_node(self, node: impl Node<P> + Information) -> Self {
        let access = AccessType::AnyShaderWrite;
        self.access_node(node, access)
    }
}

pub struct PipelinePassRef<'a, T, P>
where
    T: Access,
    P: SharedPointerKind + Send,
{
    __: PhantomData<T>,
    pass: PassRef<'a, P>,
}

impl<'a, T, P> PipelinePassRef<'a, T, P>
where
    T: Access,
    P: SharedPointerKind + Send + 'static,
{
    pub fn access_descriptor<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        access: AccessType,
    ) -> Self
    where
        N: Information,
        N: View<P>,
        ViewType: From<<N as View<P>>::Information>,
        <N as View<P>>::Information: From<<N as Information>::Info>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = node.get(self.pass.graph);
        self.access_descriptor_as(descriptor, node, access, view_info)
    }

    pub fn access_descriptor_as<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        access: AccessType,
        view_info: impl Into<N::Information>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View<P>>::Subresource::from(<N as View<P>>::Information::clone(&view_info));

        self.access_descriptor_subrange(descriptor, node, access, view_info, subresource)
    }

    pub fn access_descriptor_subrange<N>(
        mut self,
        descriptor: impl Into<Descriptor>,
        node: N,
        access: AccessType,
        view_info: impl Into<N::Information>,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
    {
        self.pass
            .push_node_access(node, access, Some(subresource.into().into()));
        self.push_node_view_bind(node, view_info.into(), descriptor.into());

        self
    }

    pub fn access_node(mut self, node: impl Node<P>, access: AccessType) -> Self {
        self.access_node_mut(node, access);
        self
    }

    pub fn access_node_mut(&mut self, node: impl Node<P>, access: AccessType) -> &mut Self {
        self.pass.push_node_access(node, access, None);
        self
    }

    fn push_node_view_bind(
        &mut self,
        node: impl Node<P>,
        view_info: impl Into<ViewType>,
        binding: Descriptor,
    ) {
        let node_idx = node.index();
        self.pass.assert_bound_graph_node(node);

        assert!(
            self.pass
                .as_mut()
                .execs
                .last_mut()
                .unwrap()
                .bindings
                .insert(binding, (node_idx, Some(view_info.into())))
                .is_none(),
            "Descriptor {binding:?} has already been bound"
        );
    }

    pub fn read_descriptor<N>(self, descriptor: impl Into<Descriptor>, node: N) -> Self
    where
        N: Information,
        N: View<P>,
        ViewType: From<<N as View<P>>::Information>,
        <N as View<P>>::Information: From<<N as Information>::Info>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = node.get(self.pass.graph);
        self.read_descriptor_as(descriptor, node, view_info)
    }

    pub fn read_descriptor_as<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        view_info: impl Into<N::Information>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View<P>>::Subresource::from(<N as View<P>>::Information::clone(&view_info));

        self.read_descriptor_subrange(descriptor, node, view_info, subresource)
    }

    pub fn read_descriptor_subrange<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        view_info: impl Into<N::Information>,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
    {
        let access = <T as Access>::DEFAULT_READ;
        self.access_descriptor_subrange(descriptor, node, access, view_info, subresource)
    }

    pub fn read_node(self, node: impl Node<P>) -> Self {
        let access = <T as Access>::DEFAULT_READ;
        self.access_node(node, access)
    }

    pub fn submit_pass(self) -> &'a mut RenderGraph<P> {
        self.pass.submit_pass()
    }

    pub fn write_descriptor<N>(self, descriptor: impl Into<Descriptor>, node: N) -> Self
    where
        N: Information,
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
        <N as View<P>>::Information: From<<N as Information>::Info>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = node.get(self.pass.graph);
        self.write_descriptor_as(descriptor, node, view_info)
    }

    pub fn write_descriptor_as<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        view_info: impl Into<N::Information>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
        <N as View<P>>::Subresource: From<<N as View<P>>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View<P>>::Subresource::from(<N as View<P>>::Information::clone(&view_info));

        self.write_descriptor_subrange(descriptor, node, view_info, subresource)
    }

    pub fn write_descriptor_subrange<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        view_info: impl Into<N::Information>,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View<P>,
        <N as View<P>>::Information: Into<ViewType>,
    {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_descriptor_subrange(descriptor, node, access, view_info, subresource)
    }

    pub fn write_node(self, node: impl Node<P>) -> Self {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_node(node, access)
    }
}

impl<'a, P> PipelinePassRef<'a, ComputePipeline<P>, P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn dispatch(mut self, group_count_x: u32, group_count_y: u32, group_count_z: u32) -> Self {
        let pass = self.pass.as_mut();
        let push_consts = take(&mut pass.push_consts);
        let pipeline = pass
            .execs
            .last_mut()
            .unwrap()
            .pipeline
            .as_ref()
            .unwrap()
            .unwrap_compute();
        let layout = pipeline.layout;

        self.pass.push_execute(move |device, cmd_buf, _| unsafe {
            push_constants(device, cmd_buf, push_consts, layout);

            device.cmd_dispatch(cmd_buf, group_count_x, group_count_y, group_count_z);
        });

        self
    }

    pub fn dispatch_indirect(mut self, args_buf: BufferNode<P>, args_buf_offset: u64) -> Self {
        let pass = self.pass.as_mut();
        let push_consts = take(&mut pass.push_consts);
        let pipeline = pass
            .execs
            .last_mut()
            .unwrap()
            .pipeline
            .as_ref()
            .unwrap()
            .unwrap_compute();
        let layout = pipeline.layout;

        self.pass
            .push_execute(move |device, cmd_buf, bindings| unsafe {
                push_constants(device, cmd_buf, push_consts, layout);

                device.cmd_dispatch_indirect(cmd_buf, *bindings[args_buf], args_buf_offset);
            });

        self
    }

    pub fn push_constants(self, data: impl Sized) -> Self {
        self.push_constants_offset(0, data)
    }

    pub fn push_constants_offset(mut self, offset: u32, data: impl Sized) -> Self {
        let data = any_as_u8_slice(&data).to_vec();
        self.pass.as_mut().push_consts.push(PushConstantRange {
            data,
            offset,
            stage: vk::ShaderStageFlags::COMPUTE,
        });

        self
    }
}

impl<'a, P> PipelinePassRef<'a, GraphicPipeline<P>, P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn attach_color(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.attach_color_as(attachment, image, image_view_info)
    }

    pub fn attach_color_as(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = view_info.into();
        self.load_color_as(attachment, image, image_view_info)
            .store_color_as(attachment, image, image_view_info)
    }

    pub fn attach_depth_stencil(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.attach_depth_stencil_as(attachment, image, image_view_info)
    }

    pub fn attach_depth_stencil_as(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        self.load_depth_stencil_as(attachment, image, image_view_info)
            .store_depth_stencil_as(attachment, image, image_view_info)
    }

    pub fn clear_color(self, attachment: AttachmentIndex) -> Self {
        self.clear_color_value(attachment, Default::default())
    }

    pub fn clear_color_value(
        mut self,
        attachment: AttachmentIndex,
        color_value: vk::ClearColorValue,
    ) -> Self {
        assert!(self
            .pass
            .as_ref()
            .load_attachments
            .attached
            .get(attachment as usize)
            .is_none());

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .clears
            .insert(attachment, vk::ClearValue { color: color_value });
        self
    }

    pub fn clear_depth_stencil(self, attachment: AttachmentIndex) -> Self {
        self.clear_depth_stencil_value(attachment, Default::default())
    }

    pub fn clear_depth_stencil_value(
        mut self,
        attachment: AttachmentIndex,
        depth_stencil_value: vk::ClearDepthStencilValue,
    ) -> Self {
        let pass = self.pass.as_mut();

        assert!(pass.load_attachments.depth_stencil.is_none());
        assert!(pass
            .load_attachments
            .attached
            .get(attachment as usize)
            .is_none());

        self.pass.as_mut().execs.last_mut().unwrap().clears.insert(
            attachment,
            vk::ClearValue {
                depth_stencil: depth_stencil_value,
            },
        );
        self
    }

    pub fn clear_scissor(mut self) -> Self {
        self.pass.as_mut().scissor = None;
        self
    }

    pub fn draw(
        mut self,
        func: impl FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send + 'static,
    ) -> Self {
        use std::slice::from_ref;

        let pass = self.pass.as_mut();
        let push_consts = take(&mut pass.push_consts);
        let scissor = take(&mut pass.scissor);
        let viewport = take(&mut pass.viewport);
        let pipeline = pass
            .execs
            .last_mut()
            .unwrap()
            .pipeline
            .as_ref()
            .unwrap()
            .unwrap_graphic();
        let layout = pipeline.layout;

        self.pass
            .push_execute(move |device, cmd_buf, bindings| unsafe {
                push_constants(device, cmd_buf, push_consts, layout);

                if let Some((area, depth)) = viewport {
                    device.cmd_set_viewport(
                        cmd_buf,
                        0,
                        from_ref(&vk::Viewport {
                            x: area.offset.x,
                            y: area.offset.y,
                            width: area.extent.x,
                            height: area.extent.y,
                            min_depth: depth.start,
                            max_depth: depth.end,
                        }),
                    );
                }

                if let Some(area) = scissor {
                    device.cmd_set_scissor(
                        cmd_buf,
                        0,
                        from_ref(&vk::Rect2D {
                            extent: vk::Extent2D {
                                width: area.extent.x,
                                height: area.extent.y,
                            },
                            offset: vk::Offset2D {
                                x: area.offset.x,
                                y: area.offset.y,
                            },
                        }),
                    );
                }

                func(device, cmd_buf, bindings);
            });
        self
    }

    fn image_info(&self, node_idx: NodeIndex) -> (vk::Format, SampleCount) {
        let image_info = &self.pass.graph.bindings[node_idx].as_image_info().unwrap();

        (image_info.fmt, image_info.sample_count)
    }

    pub fn load_color(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.attach_color_as(attachment, image, image_view_info)
    }

    pub fn load_color_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.load_attachments.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let color_attachment = pass
                .load_attachments
                .attached
                .get(attachment as usize)
                .unwrap()
                .unwrap();

            assert!(self
                .pass
                .as_ref()
                .store_attachments
                .attached
                .get(attachment as usize)
                .map(|stored_attachment| Attachment::are_compatible(
                    *stored_attachment,
                    Some(color_attachment)
                ))
                .unwrap_or(true));
            assert!(self
                .pass
                .as_ref()
                .resolve_attachments
                .attached
                .get(attachment as usize)
                .map(|resolved_attachment| Attachment::are_compatible(
                    *resolved_attachment,
                    Some(color_attachment)
                ))
                .unwrap_or(true));
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentRead,
            Some(Subresource::Image(image_view_info.into())),
        );
        self
    }

    pub fn load_depth_stencil(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.load_depth_stencil_as(attachment, image, image_view_info)
    }

    pub fn load_depth_stencil_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.load_attachments.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let (_, loaded_attachment) = pass.load_attachments.depth_stencil().unwrap();

            assert!(self
                .pass
                .as_ref()
                .store_attachments
                .depth_stencil()
                .map(
                    |(attachment_idx, stored_attachment)| attachment == attachment_idx
                        && Attachment::are_identical(stored_attachment, loaded_attachment)
                )
                .unwrap_or(true));
            assert!(self
                .pass
                .as_ref()
                .resolve_attachments
                .depth_stencil()
                .map(
                    |(attachment_idx, resolved_attachment)| attachment == attachment_idx
                        && Attachment::are_identical(resolved_attachment, loaded_attachment)
                )
                .unwrap_or(true));
        }

        self.pass.push_node_access(
            image,
            AccessType::DepthStencilAttachmentRead,
            Some(Subresource::Image(image_view_info.into())),
        );
        self
    }

    fn pipeline(&self) -> &GraphicPipeline<P> {
        let exec = self.pass.as_ref().execs.last().unwrap();
        let pipeline = exec.pipeline.as_ref().unwrap();

        pipeline.unwrap_graphic()
    }

    fn pipeline_stages(pipeline: &GraphicPipeline<P>) -> vk::ShaderStageFlags {
        pipeline
            .state
            .stages
            .iter()
            .map(|stage| stage.flags)
            .reduce(|j, k| j | k)
            .unwrap_or_default()
    }

    pub fn push_constants(self, data: impl Sized) -> Self {
        let pipeline = self.pipeline();
        let whole_stage = Self::pipeline_stages(pipeline);
        self.push_stage_constants(0, whole_stage, data)
    }

    pub fn push_stage_constants(
        mut self,
        offset: u32,
        mut stage: vk::ShaderStageFlags,
        data: impl Sized,
    ) -> Self {
        let pipeline = self.pipeline();
        let whole_stage = Self::pipeline_stages(pipeline);

        if stage & whole_stage != stage {
            warn!("extra stage flags specified");

            stage &= whole_stage;
        }

        let data = any_as_u8_slice(&data);
        let mut push_consts = vec![];
        for range in &pipeline.push_constant_ranges {
            let stage = range.stage_flags & stage;
            if !stage.is_empty()
                && offset <= range.offset
                && offset as usize + data.len() > range.offset as usize
            {
                let start = (range.offset - offset) as usize;
                let end = range.offset as usize + (data.len() - start).min(range.size as usize);
                let data = data[start..end].to_vec();

                // trace!("Push constant {:?} {}..{}", stage, start, end);

                push_consts.push(PushConstantRange {
                    data,
                    offset: range.offset,
                    stage,
                });
            }
        }

        self.pass
            .as_mut()
            .push_consts
            .extend(push_consts.into_iter());

        self
    }

    pub fn resolve_color(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.resolve_color_as(attachment, image, image_view_info)
    }

    pub fn resolve_color_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.resolve_attachments.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let resolved_attachment = pass
                .resolve_attachments
                .attached
                .get(attachment as usize)
                .unwrap()
                .unwrap();

            assert!(self
                .pass
                .as_ref()
                .load_attachments
                .attached
                .get(attachment as usize)
                .map(|loaded_attachment| Attachment::are_compatible(
                    *loaded_attachment,
                    Some(resolved_attachment)
                ))
                .unwrap_or(true));
            assert!(self
                .pass
                .as_ref()
                .store_attachments
                .attached
                .get(attachment as usize)
                .is_none());
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );
        self
    }

    pub fn resolve_depth_stencil(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.resolve_depth_stencil_as(attachment, image, image_view_info)
    }

    pub fn resolve_depth_stencil_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.resolve_attachments.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let (_, resolved_attachment) = pass.resolve_attachments.depth_stencil().unwrap();

            assert!(self
                .pass
                .as_ref()
                .load_attachments
                .depth_stencil()
                .map(
                    |(attachment_idx, loaded_attachment)| attachment == attachment_idx
                        && Attachment::are_identical(loaded_attachment, resolved_attachment)
                )
                .unwrap_or(true));
            assert!(self.pass.as_ref().store_attachments.depth_stencil.is_none());
        }

        self.pass.push_node_access(
            image,
            if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::STENCIL)
            {
                AccessType::DepthStencilAttachmentWrite
            } else {
                AccessType::DepthAttachmentWriteStencilReadOnly
            },
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    // This can only be called once per pass.
    pub fn set_depth_stencil(mut self, depth_stencil: DepthStencilMode) -> Self {
        let pass = self.pass.as_mut();

        assert!(pass.depth_stencil.is_none());

        pass.depth_stencil = Some(depth_stencil);
        self
    }

    // This can only be called once per pass! last value wins
    pub fn set_render_area(mut self, x: i32, y: i32, width: u32, height: u32) -> Self {
        self.pass.as_mut().render_area = Some(Rect {
            extent: uvec2(width, height),
            offset: ivec2(x, y),
        });
        self
    }

    pub fn set_scissor(mut self, x: i32, y: i32, width: u32, height: u32) -> Self {
        self.pass.as_mut().scissor = Some(Rect {
            extent: uvec2(width, height),
            offset: ivec2(x, y),
        });
        self
    }

    pub fn set_viewport(
        mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        depth: Range<f32>,
    ) -> Self {
        self.pass.as_mut().viewport = Some((
            Rect {
                extent: vec2(width, height),
                offset: vec2(x, y),
            },
            depth,
        ));
        self
    }

    pub fn store_color(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.store_color_as(attachment, image, image_view_info)
    }

    pub fn store_color_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.store_attachments.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let stored_attachment = pass
                .store_attachments
                .attached
                .get(attachment as usize)
                .unwrap()
                .unwrap();

            assert!(self
                .pass
                .as_ref()
                .load_attachments
                .attached
                .get(attachment as usize)
                .map(|loaded_attachment| Attachment::are_compatible(
                    *loaded_attachment,
                    Some(stored_attachment)
                ))
                .unwrap_or(true));
            assert!(self
                .pass
                .as_ref()
                .resolve_attachments
                .attached
                .get(attachment as usize)
                .is_none());
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    pub fn store_depth_stencil(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
    ) -> Self {
        let image: AnyImageNode<P> = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();
        self.store_depth_stencil_as(attachment, image, image_view_info)
    }

    pub fn store_depth_stencil_as(
        mut self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode<P>>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();

            assert!(pass.store_attachments.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            let (_, stored_attachment) = pass.store_attachments.depth_stencil().unwrap();

            assert!(self
                .pass
                .as_ref()
                .load_attachments
                .depth_stencil()
                .map(
                    |(attachment_idx, loaded_attachment)| attachment == attachment_idx
                        && Attachment::are_identical(loaded_attachment, stored_attachment)
                )
                .unwrap_or(true));
            assert!(self
                .pass
                .as_ref()
                .resolve_attachments
                .depth_stencil
                .is_none());
        }

        self.pass.push_node_access(
            image,
            if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::STENCIL)
            {
                AccessType::DepthStencilAttachmentWrite
            } else {
                AccessType::DepthAttachmentWriteStencilReadOnly
            },
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }
}

impl<'a, P> PipelinePassRef<'a, RayTracePipeline<P>, P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn push_constants(self, data: impl Sized) -> Self {
        // TODO: Flags need limiting
        self.push_stage_constants(0, vk::ShaderStageFlags::ALL, data)
    }

    pub fn push_stage_constants(
        mut self,
        offset: u32,
        stage: vk::ShaderStageFlags,
        data: impl Sized,
    ) -> Self {
        let data = any_as_u8_slice(&data).to_vec();
        self.pass.as_mut().push_consts.push(PushConstantRange {
            data,
            offset,
            stage,
        });
        self
    }

    pub fn trace_rays(self, _tlas: RayTraceAccelerationNode<P>, _extent: UVec3) -> Self {
        // let mut pass = self.pass.as_mut();
        // let push_consts = take(&mut pass.push_consts);
        // let pipeline = Shared::clone(pass.pipelines.get(0).unwrap().unwrap_ray_trace());
        // let layout = pipeline.layout;

        // // TODO: Bind op to get a descriptor?

        // self.pass.push_execute(move |cmd_buf, bindings| unsafe {
        //     push_constants(push_consts, cmd_buf, layout);

        //     cmd_buf.device.ray_trace_pipeline_ext.cmd_trace_rays(
        //         **cmd_buf,
        //         &pipeline.shader_bindings.raygen,
        //         &pipeline.shader_bindings.miss,
        //         &pipeline.shader_bindings.hit,
        //         &pipeline.shader_bindings.callable,
        //         extent.x,
        //         extent.y,
        //         extent.z,
        //     );
        // });
        self
    }

    pub fn trace_rays_indirect(
        self,
        _tlas: RayTraceAccelerationNode<P>,
        _args_buf: BufferNode<P>,
        _args_buf_offset: u64,
    ) -> Self {
        // let mut pass = self.pass.as_mut();
        // let push_consts = take(&mut pass.push_consts);
        // let pipeline = Shared::clone(pass.pipelines.get(0).unwrap().unwrap_ray_trace());
        // let layout = pipeline.layout;

        // // TODO: Bind op to get a descriptor?

        // self.pass.push_execute(move |cmd_buf, bindings| unsafe {
        //     push_constants(push_consts, cmd_buf, layout);

        //     let args_buf_address = Buffer::device_address(&bindings[args_buf]) + args_buf_offset;
        //     cmd_buf
        //         .device
        //         .ray_trace_pipeline_ext
        //         .cmd_trace_rays_indirect(
        //             **cmd_buf,
        //             from_ref(&pipeline.shader_bindings.raygen),
        //             from_ref(&pipeline.shader_bindings.miss),
        //             from_ref(&pipeline.shader_bindings.hit),
        //             from_ref(&pipeline.shader_bindings.callable),
        //             args_buf_address,
        //         );
        // });
        self
    }
}
