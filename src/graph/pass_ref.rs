use {
    super::{
        AnyBufferNode, AnyImageNode, Area, Attachment, AttachmentIndex, Bind, Binding,
        BufferLeaseNode, BufferNode, Color, Descriptor, Edge, Execution, ExecutionFunction,
        ExecutionPipeline, ImageLeaseNode, ImageNode, Information, Node, NodeIndex, Pass,
        RayTraceAccelerationNode, RenderGraph, SampleCount, Subresource, SubresourceAccess,
        SwapchainImageNode, View, ViewType,
    },
    crate::{
        driver::{
            Buffer, ComputePipeline, DepthStencilMode, GraphicPipeline, Image, ImageViewInfo,
            RayTraceAcceleration, RayTracePipeline,
        },
        into_u8_slice,
        ptr::Shared,
    },
    archery::SharedPointerKind,
    ash::vk,
    log::{trace, warn},
    std::{
        marker::PhantomData,
        mem::size_of_val,
        ops::{Index, Range},
    },
    vk_sync::AccessType,
};

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
            "unexpected node access: call access, read, or write first"
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

pub struct Compute<'a, P>
where
    P: SharedPointerKind,
{
    bindings: Bindings<'a, P>,
    cmd_buf: vk::CommandBuffer,
    device: &'a ash::Device,
    pipeline: Shared<ComputePipeline<P>, P>,
}

impl<'a, P> Compute<'a, P>
where
    P: SharedPointerKind,
{
    pub fn dispatch(
        &mut self,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
    ) -> &mut Self {
        unsafe {
            self.device
                .cmd_dispatch(self.cmd_buf, group_count_x, group_count_y, group_count_z);
        }

        self
    }

    pub fn dispatch_base(
        &mut self,
        base_group_x: u32,
        base_group_y: u32,
        base_group_z: u32,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
    ) -> &mut Self {
        unsafe {
            self.device.cmd_dispatch_base(
                self.cmd_buf,
                base_group_x,
                base_group_y,
                base_group_z,
                group_count_x,
                group_count_y,
                group_count_z,
            );
        }

        self
    }

    pub fn dispatch_indirect(
        &mut self,
        args_buf: impl Into<AnyBufferNode<P>>,
        args_offset: vk::DeviceSize,
    ) -> &mut Self {
        let args_buf = args_buf.into();

        unsafe {
            self.device
                .cmd_dispatch_indirect(self.cmd_buf, *self.bindings[args_buf], args_offset);
        }

        self
    }

    pub fn push_constants(&mut self, data: impl Sized) -> &mut Self {
        self.push_constants_offset(0, data)
    }

    pub fn push_constants_offset(&mut self, offset: u32, data: impl Sized) -> &mut Self {
        use std::slice::from_ref;

        let data = into_u8_slice(from_ref(&data));
        if let Some(push_const) = &self.pipeline.push_constants {
            // Determine the range of the overall pipline push constants which overlap with `data`
            let push_const_end = push_const.offset + push_const.size;
            let data_end = offset + data.len() as u32;
            let end = data_end.min(push_const_end);
            let start = offset.max(push_const.offset);

            if end > start {
                trace!(
                    "      push constants {:?} {}..{}",
                    push_const.stage_flags,
                    start,
                    end
                );

                unsafe {
                    self.device.cmd_push_constants(
                        self.cmd_buf,
                        self.pipeline.layout,
                        vk::ShaderStageFlags::COMPUTE,
                        push_const.offset,
                        &data[(start - offset) as usize..(end - offset) as usize],
                    );
                }
            }
        }

        self
    }
}

pub struct Draw<'a, P>
where
    P: SharedPointerKind,
{
    bindings: Bindings<'a, P>,
    buffers: &'a mut Vec<vk::Buffer>,
    cmd_buf: vk::CommandBuffer,
    device: &'a ash::Device,
    offsets: &'a mut Vec<vk::DeviceSize>,
    pipeline: Shared<GraphicPipeline<P>, P>,
    rects: &'a mut Vec<vk::Rect2D>,
    viewports: &'a mut Vec<vk::Viewport>,
}

impl<'a, P> Draw<'a, P>
where
    P: SharedPointerKind,
{
    pub fn bind_index_buffer(
        &mut self,
        buf: impl Into<AnyBufferNode<P>>,
        index_ty: vk::IndexType,
    ) -> &mut Self {
        self.bind_index_buffer_offset(buf, index_ty, 0)
    }

    pub fn bind_index_buffer_offset(
        &mut self,
        buf: impl Into<AnyBufferNode<P>>,
        index_ty: vk::IndexType,
        offset: vk::DeviceSize,
    ) -> &mut Self {
        let buf = buf.into();

        unsafe {
            self.device
                .cmd_bind_index_buffer(self.cmd_buf, *self.bindings[buf], offset, index_ty);
        }

        self
    }

    pub fn bind_vertex_buffer(&mut self, buffer: impl Into<AnyBufferNode<P>>) -> &mut Self {
        self.bind_vertex_buffer_offset(buffer, 0)
    }

    pub fn bind_vertex_buffer_offset(
        &mut self,
        buffer: impl Into<AnyBufferNode<P>>,
        offset: vk::DeviceSize,
    ) -> &mut Self {
        use std::slice::from_ref;

        let buffer = buffer.into();

        unsafe {
            self.device.cmd_bind_vertex_buffers(
                self.cmd_buf,
                0,
                from_ref(&self.bindings[buffer]),
                from_ref(&offset),
            );
        }

        self
    }

    pub fn bind_vertex_buffers<B>(
        &mut self,
        first_binding: u32,
        buffers: impl IntoIterator<Item = (B, vk::DeviceSize)>,
    ) -> &mut Self
    where
        B: Into<AnyBufferNode<P>>,
    {
        self.buffers.clear();
        self.offsets.clear();

        for (buffer, offset) in buffers {
            let buffer = buffer.into();

            self.buffers.push(*self.bindings[buffer]);
            self.offsets.push(offset);
        }

        unsafe {
            self.device.cmd_bind_vertex_buffers(
                self.cmd_buf,
                first_binding,
                self.buffers,
                self.offsets,
            );
        }

        self
    }

    pub fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> &mut Self {
        unsafe {
            self.device.cmd_draw(
                self.cmd_buf,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            );
        }

        self
    }

    pub fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) -> &mut Self {
        unsafe {
            self.device.cmd_draw_indexed(
                self.cmd_buf,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            );
        }
        self
    }

    pub fn draw_indexed_indirect(
        &mut self,
        buffer: impl Into<AnyBufferNode<P>>,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) -> &mut Self {
        let buffer = buffer.into();

        unsafe {
            self.device.cmd_draw_indexed_indirect(
                self.cmd_buf,
                *self.bindings[buffer],
                offset,
                draw_count,
                stride,
            );
        }
        self
    }

    pub fn draw_indexed_indirect_count(
        &mut self,
        buffer: impl Into<AnyBufferNode<P>>,
        offset: vk::DeviceSize,
        count_buf: impl Into<AnyBufferNode<P>>,
        count_buf_offset: vk::DeviceSize,
        max_draw_count: u32,
        stride: u32,
    ) -> &mut Self {
        let buffer = buffer.into();
        let count_buf = count_buf.into();

        unsafe {
            self.device.cmd_draw_indexed_indirect_count(
                self.cmd_buf,
                *self.bindings[buffer],
                offset,
                *self.bindings[count_buf],
                count_buf_offset,
                max_draw_count,
                stride,
            );
        }
        self
    }

    pub fn draw_indirect(
        &mut self,
        buffer: impl Into<AnyBufferNode<P>>,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) -> &mut Self {
        let buffer = buffer.into();

        unsafe {
            self.device.cmd_draw_indirect(
                self.cmd_buf,
                *self.bindings[buffer],
                offset,
                draw_count,
                stride,
            );
        }
        self
    }

    pub fn draw_indirect_count(
        &mut self,
        buffer: impl Into<AnyBufferNode<P>>,
        offset: vk::DeviceSize,
        count_buf: impl Into<AnyBufferNode<P>>,
        count_buf_offset: vk::DeviceSize,
        max_draw_count: u32,
        stride: u32,
    ) -> &mut Self {
        let buffer = buffer.into();
        let count_buf = count_buf.into();

        unsafe {
            self.device.cmd_draw_indirect_count(
                self.cmd_buf,
                *self.bindings[buffer],
                offset,
                *self.bindings[count_buf],
                count_buf_offset,
                max_draw_count,
                stride,
            );
        }
        self
    }

    pub fn push_constants(&mut self, data: impl Sized) -> &mut Self {
        let data_size = size_of_val(&data);

        // Specify each stage that has a push constant in this region
        let mut stage_flags = vk::ShaderStageFlags::empty();
        for push_const in &self.pipeline.push_constants {
            if (push_const.offset as usize) < data_size {
                stage_flags |= push_const.stage_flags;
            }
        }

        self.push_constants_offset(stage_flags, 0, data)
    }

    pub fn push_constants_offset(
        &mut self,
        mut stage_flags: vk::ShaderStageFlags,
        offset: u32,
        data: impl Sized,
    ) -> &mut Self {
        use std::slice::from_ref;

        // Check if the specified stages are a super-set of our actual stages
        if stage_flags != stage_flags & self.pipeline.stages() {
            // The user has manually specified too many stages
            warn!("      unused shader stage flags");

            // Remove extra stages
            stage_flags &= self.pipeline.stages();
        }

        let data = into_u8_slice(from_ref(&data));
        for push_const in &self.pipeline.push_constants {
            // Determine the range of the overall pipline push constants which overlap with `data`
            let push_const_end = push_const.offset + push_const.size;
            let data_end = offset + data.len() as u32;
            let end = data_end.min(push_const_end);
            let start = offset.max(push_const.offset);

            if end > start {
                trace!(
                    "      push constants {:?} {}..{}",
                    push_const.stage_flags,
                    start,
                    end
                );

                unsafe {
                    self.device.cmd_push_constants(
                        self.cmd_buf,
                        self.pipeline.layout,
                        push_const.stage_flags,
                        start,
                        &data[(start - offset) as usize..(end - offset) as usize],
                    );
                }
            }
        }

        self
    }

    pub fn set_scissor(&mut self, x: i32, y: i32, width: u32, height: u32) -> &mut Self {
        unsafe {
            self.device.cmd_set_scissor(
                self.cmd_buf,
                0,
                &[vk::Rect2D {
                    extent: vk::Extent2D { width, height },
                    offset: vk::Offset2D { x, y },
                }],
            );
        }

        self
    }

    pub fn set_scissors<S>(
        &mut self,
        first_scissor: u32,
        scissors: impl IntoIterator<Item = S>,
    ) -> &mut Self
    where
        S: Into<vk::Rect2D>,
    {
        self.rects.clear();

        for scissor in scissors {
            self.rects.push(scissor.into());
        }

        unsafe {
            self.device
                .cmd_set_scissor(self.cmd_buf, first_scissor, self.rects);
        }

        self
    }

    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        depth: Range<f32>,
    ) -> &mut Self {
        unsafe {
            self.device.cmd_set_viewport(
                self.cmd_buf,
                0,
                &[vk::Viewport {
                    x,
                    y,
                    width,
                    height,
                    min_depth: depth.start,
                    max_depth: depth.end,
                }],
            );
        }

        self
    }

    pub fn set_viewports<V>(
        &mut self,
        first_viewport: u32,
        viewports: impl IntoIterator<Item = V>,
    ) -> &mut Self
    where
        V: Into<vk::Viewport>,
    {
        self.viewports.clear();

        for viewport in viewports {
            self.viewports.push(viewport.into());
        }

        unsafe {
            self.device
                .cmd_set_viewport(self.cmd_buf, first_viewport, self.viewports);
        }

        self
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
            depth_stencil: None,
            execs: vec![Default::default()], // We start off with a default execution!
            name,
            render_area: None,
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

    fn push_execute(
        &mut self,
        func: impl FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send + 'static,
    ) {
        let pass = self.as_mut();
        let exec = {
            let last_exec = pass.execs.last_mut().unwrap();
            last_exec.func = Some(ExecutionFunction(Box::new(func)));

            Execution {
                pipeline: last_exec.pipeline.clone(),
                loads: last_exec.loads.clone(),
                resolves: last_exec.resolves.clone(),
                stores: last_exec.stores.clone(),
                ..Default::default()
            }
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

    pub fn record_cmd_buf(
        mut self,
        func: impl FnOnce(&ash::Device, vk::CommandBuffer, Bindings<'_, P>) + Send + 'static,
    ) -> Self {
        self.push_execute(func);
        self
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
    pub fn record_compute(
        mut self,
        func: impl FnOnce(&mut Compute<'_, P>) + Send + 'static,
    ) -> Self {
        let pipeline = Shared::clone(
            self.pass
                .as_ref()
                .execs
                .last()
                .unwrap()
                .pipeline
                .as_ref()
                .unwrap()
                .unwrap_compute(),
        );

        self.pass.push_execute(move |device, cmd_buf, bindings| {
            func(&mut Compute {
                bindings,
                cmd_buf,
                device,
                pipeline,
            });
        });

        self
    }
}

impl<'a, P> PipelinePassRef<'a, GraphicPipeline<P>, P>
where
    P: SharedPointerKind + Send + 'static,
{
    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
    ///
    /// The image is
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

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
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

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
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

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
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

    /// Clears the render pass attachment of any existing data.
    pub fn clear_color(self, attachment: AttachmentIndex) -> Self {
        self.clear_color_value(attachment, [0, 0, 0, 0])
    }

    /// Clears the render pass attachment of any existing data.
    pub fn clear_color_value(
        mut self,
        attachment: AttachmentIndex,
        color: impl Into<Color>,
    ) -> Self {
        let color = color.into();
        let pass = self.pass.as_mut();
        let exec = pass.execs.last_mut().unwrap();

        debug_assert!(exec.loads.attached.get(attachment as usize).is_none());

        exec.clears.insert(
            attachment,
            vk::ClearValue {
                color: vk::ClearColorValue { float32: color.0 },
            },
        );

        self
    }

    /// Clears the render pass attachment of any existing data.
    pub fn clear_depth_stencil(self, attachment: AttachmentIndex) -> Self {
        self.clear_depth_stencil_value(attachment, 0.0, 0)
    }

    /// Clears the render pass attachment of any existing data.
    pub fn clear_depth_stencil_value(
        mut self,
        attachment: AttachmentIndex,
        depth_value: f32,
        stencil_value: u32,
    ) -> Self {
        let pass = self.pass.as_mut();
        let exec = pass.execs.last_mut().unwrap();

        debug_assert!(exec.loads.depth_stencil.is_none());
        debug_assert!(exec.loads.attached.get(attachment as usize).is_none());

        exec.clears.insert(
            attachment,
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: depth_value,
                    stencil: stencil_value,
                },
            },
        );

        self
    }

    fn image_info(&self, node_idx: NodeIndex) -> (vk::Format, SampleCount) {
        let image_info = &self.pass.graph.bindings[node_idx].as_image_info().unwrap();

        (image_info.fmt, image_info.sample_count)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
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

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.loads.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let color_attachment = exec
                    .loads
                    .attached
                    .get(attachment as usize)
                    .copied()
                    .flatten()
                    .unwrap();

                assert!(exec
                    .stores
                    .attached
                    .get(attachment as usize)
                    .map(|stored_attachment| Attachment::are_compatible(
                        *stored_attachment,
                        Some(color_attachment)
                    ))
                    .unwrap_or(true));
                assert!(exec
                    .resolves
                    .attached
                    .get(attachment as usize)
                    .map(|resolved_attachment| Attachment::are_compatible(
                        *resolved_attachment,
                        Some(color_attachment)
                    ))
                    .unwrap_or(true));
            }
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentRead,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
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

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.loads.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, loaded_attachment) = exec.loads.depth_stencil().unwrap();

                assert!(exec
                    .stores
                    .depth_stencil()
                    .map(
                        |(attachment_idx, stored_attachment)| attachment == attachment_idx
                            && Attachment::are_identical(stored_attachment, loaded_attachment)
                    )
                    .unwrap_or(true));
                assert!(exec
                    .resolves
                    .depth_stencil()
                    .map(
                        |(attachment_idx, resolved_attachment)| attachment == attachment_idx
                            && Attachment::are_identical(resolved_attachment, loaded_attachment)
                    )
                    .unwrap_or(true));
            }
        }

        self.pass.push_node_access(
            image,
            AccessType::DepthStencilAttachmentRead,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Append a graphic subpass onto the current pass of the parent render graph.
    pub fn record_subpass(mut self, func: impl FnOnce(&mut Draw<'_, P>) + Send + 'static) -> Self {
        let pipeline = Shared::clone(
            self.pass
                .as_ref()
                .execs
                .last()
                .unwrap()
                .pipeline
                .as_ref()
                .unwrap()
                .unwrap_graphic(),
        );

        self.pass.push_execute(move |device, cmd_buf, bindings| {
            use std::cell::RefCell;

            #[derive(Default)]
            struct Tls {
                buffers: Vec<vk::Buffer>,
                offsets: Vec<vk::DeviceSize>,
                rects: Vec<vk::Rect2D>,
                viewports: Vec<vk::Viewport>,
            }

            thread_local! {
                static TLS: RefCell<Tls> = Default::default();
            }

            TLS.with(|tls| {
                let Tls {
                    buffers,
                    offsets,
                    rects,
                    viewports,
                } = &mut *tls.borrow_mut();

                func(&mut Draw {
                    bindings,
                    buffers,
                    cmd_buf,
                    device,
                    offsets,
                    pipeline,
                    rects,
                    viewports,
                });
            });
        });

        self
    }

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after load.
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

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after load.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.resolves.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let resolved_attachment = exec
                    .resolves
                    .attached
                    .get(attachment as usize)
                    .copied()
                    .flatten()
                    .unwrap();

                assert!(exec
                    .loads
                    .attached
                    .get(attachment as usize)
                    .map(|loaded_attachment| Attachment::are_compatible(
                        *loaded_attachment,
                        Some(resolved_attachment)
                    ))
                    .unwrap_or(true));
                assert!(exec.stores.attached.get(attachment as usize).is_none());
            }
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after load.
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

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after load.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.resolves.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, resolved_attachment) = exec.resolves.depth_stencil().unwrap();

                assert!(exec
                    .loads
                    .depth_stencil()
                    .map(
                        |(attachment_idx, loaded_attachment)| attachment == attachment_idx
                            && Attachment::are_identical(loaded_attachment, resolved_attachment)
                    )
                    .unwrap_or(true));
                assert!(exec.stores.depth_stencil.is_none());
            }
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

    /// Sets a particular depth/stencil mode.
    ///
    /// The default depth/stencil mode is:
    ///
    /// ```
    /// # use ash::vk;
    /// # use ordered_float::OrderedFloat;
    /// # use screen_13::driver::{DepthStencilMode, StencilMode};
    /// DepthStencilMode {
    ///     back: StencilMode::Noop,
    ///     front: StencilMode::Noop,
    ///     bounds_test: false,
    ///     depth_test: true,
    ///     depth_write: true,
    ///     stencil_test: false,
    ///     compare_op: vk::CompareOp::GREATER_OR_EQUAL,
    ///     min: OrderedFloat(0.0),
    ///     max: OrderedFloat(1.0),
    /// }
    /// # ;()
    /// ```
    pub fn set_depth_stencil(&mut self, depth_stencil: DepthStencilMode) -> &mut Self {
        let pass = self.pass.as_mut();

        assert!(pass.depth_stencil.is_none());

        pass.depth_stencil = Some(depth_stencil);

        self
    }

    /// Sets the `[renderArea](https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkRenderPassBeginInfo.html#_c_specification)`
    /// field when beginning a render pass.
    ///
    /// NOTE: Setting this value will cause the viewport and scissor to be unset, which is not the default
    /// behavior. When this value is set you should call `set_viewport` and `set_scissor` on the subpass.
    ///
    /// If not set, this value defaults to the first loaded, resolved, or stored attachment dimensions and
    /// sets the viewport and scissor to the same values, with a `0..1` depth if not specified by
    /// `set_depth_stencil`.
    pub fn set_render_area(&mut self, x: i32, y: i32, width: u32, height: u32) -> &mut Self {
        self.pass.as_mut().render_area = Some(Area {
            height,
            width,
            x,
            y,
        });

        self
    }

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after load.
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

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after load.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.stores.insert_color(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let stored_attachment = exec
                    .stores
                    .attached
                    .get(attachment as usize)
                    .copied()
                    .flatten()
                    .unwrap();

                assert!(exec
                    .loads
                    .attached
                    .get(attachment as usize)
                    .map(|loaded_attachment| Attachment::are_compatible(
                        *loaded_attachment,
                        Some(stored_attachment)
                    ))
                    .unwrap_or(true));
                assert!(exec.resolves.attached.get(attachment as usize).is_none());
            }
        }

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after load.
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

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after load.
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
            let exec = pass.execs.last_mut().unwrap();

            assert!(exec.stores.set_depth_stencil(
                attachment,
                image_view_info.aspect_mask,
                image_view_info.fmt,
                sample_count,
                node_idx,
            ));

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, stored_attachment) = exec.stores.depth_stencil().unwrap();

                assert!(exec
                    .loads
                    .depth_stencil()
                    .map(
                        |(attachment_idx, loaded_attachment)| attachment == attachment_idx
                            && Attachment::are_identical(loaded_attachment, stored_attachment)
                    )
                    .unwrap_or(true));
                assert!(exec.resolves.depth_stencil.is_none());
            }
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
    pub fn record_ray_trace(
        mut self,
        func: impl FnOnce(&mut RayTrace<'_, P>) + Send + 'static,
    ) -> Self {
        let pipeline = Shared::clone(
            self.pass
                .as_ref()
                .execs
                .last()
                .unwrap()
                .pipeline
                .as_ref()
                .unwrap()
                .unwrap_ray_trace(),
        );

        self.pass.push_execute(move |device, cmd_buf, bindings| {
            func(&mut RayTrace {
                _bindings: bindings,
                _cmd_buf: cmd_buf,
                _device: device,
                _pipeline: pipeline,
            });
        });

        self
    }
}

pub struct RayTrace<'a, P>
where
    P: SharedPointerKind,
{
    _bindings: Bindings<'a, P>,
    _cmd_buf: vk::CommandBuffer,
    _device: &'a ash::Device,
    _pipeline: Shared<RayTracePipeline<P>, P>,
}

impl<'a, P> RayTrace<'a, P>
where
    P: SharedPointerKind,
{
    pub fn trace_rays(self, _tlas: RayTraceAccelerationNode<P>, _extent: ()) -> Self {
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
        _args_buf_offset: vk::DeviceSize,
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
