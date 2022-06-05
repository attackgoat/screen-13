use {
    super::{
        AccelerationStructureLeaseNode, AccelerationStructureNode, AnyAccelerationStructureNode,
        AnyBufferNode, AnyImageNode, Area, AttachmentIndex, Bind, Binding, BufferLeaseNode,
        BufferNode, Color, Descriptor, Edge, Execution, ExecutionFunction, ExecutionPipeline,
        ImageLeaseNode, ImageNode, Information, Node, NodeIndex, Pass, RenderGraph, SampleCount,
        Subresource, SubresourceAccess, SwapchainImageNode, View, ViewType,
    },
    crate::driver::{
        AccelerationStructure, AccelerationStructureGeometryData,
        AccelerationStructureGeometryInfo, Buffer, ComputePipeline, DepthStencilMode, Device,
        DeviceOrHostAddress, GraphicPipeline, Image, ImageViewInfo, RayTracePipeline,
    },
    ash::vk,
    log::trace,
    std::{
        cell::RefCell,
        marker::PhantomData,
        ops::{Index, Range},
        sync::Arc,
    },
    vk_sync::AccessType,
};

#[cfg(debug_assertions)]
use super::Attachment;

pub struct Acceleration<'a> {
    bindings: Bindings<'a>,
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
}

impl<'a> Acceleration<'a> {
    pub fn build_structure(
        &self,
        accel_struct_node: impl Into<AnyAccelerationStructureNode>,
        scratch_buf_node: impl Into<AnyBufferNode>,
        build_info: AccelerationStructureGeometryInfo,
        build_ranges: &[vk::AccelerationStructureBuildRangeInfoKHR],
    ) {
        use std::slice::from_ref;

        let accel_struct_node = accel_struct_node.into();
        let scratch_buf_node = scratch_buf_node.into();

        unsafe {
            #[derive(Default)]
            struct Tls {
                geometries: Vec<vk::AccelerationStructureGeometryKHR>,
                max_primitive_counts: Vec<u32>,
            }

            thread_local! {
                static TLS: RefCell<Tls> = Default::default();
            }

            TLS.with(|tls| {
                let mut tls = tls.borrow_mut();
                tls.geometries.clear();
                tls.max_primitive_counts.clear();

                for info in build_info.geometries.iter() {
                    let flags = info.flags;

                    let (geometry_type, geometry) = match &info.geometry {
                        &AccelerationStructureGeometryData::AABBs { stride } => (
                            vk::GeometryTypeKHR::AABBS,
                            vk::AccelerationStructureGeometryDataKHR {
                                aabbs: vk::AccelerationStructureGeometryAabbsDataKHR {
                                    stride,
                                    ..Default::default()
                                },
                            },
                        ),
                        &AccelerationStructureGeometryData::Instances {
                            array_of_pointers,
                            data,
                        } => (
                            vk::GeometryTypeKHR::INSTANCES,
                            vk::AccelerationStructureGeometryDataKHR {
                                instances: vk::AccelerationStructureGeometryInstancesDataKHR {
                                    array_of_pointers: array_of_pointers as _,
                                    data: match data {
                                        DeviceOrHostAddress::DeviceAddress(device_address) => {
                                            vk::DeviceOrHostAddressConstKHR { device_address }
                                        }
                                        DeviceOrHostAddress::HostAddress => todo!(),
                                    },
                                    ..Default::default()
                                },
                            },
                        ),
                        &AccelerationStructureGeometryData::Triangles {
                            index_data,
                            index_type,
                            max_vertex,
                            transform_data,
                            vertex_data,
                            vertex_format,
                            vertex_stride,
                        } => (
                            vk::GeometryTypeKHR::TRIANGLES,
                            vk::AccelerationStructureGeometryDataKHR {
                                triangles: vk::AccelerationStructureGeometryTrianglesDataKHR {
                                    index_data: match index_data {
                                        DeviceOrHostAddress::DeviceAddress(device_address) => {
                                            vk::DeviceOrHostAddressConstKHR { device_address }
                                        }
                                        DeviceOrHostAddress::HostAddress => todo!(),
                                    },
                                    index_type,
                                    max_vertex,
                                    transform_data: match transform_data {
                                        Some(DeviceOrHostAddress::DeviceAddress(
                                            device_address,
                                        )) => vk::DeviceOrHostAddressConstKHR { device_address },
                                        Some(DeviceOrHostAddress::HostAddress) => todo!(),
                                        None => {
                                            vk::DeviceOrHostAddressConstKHR { device_address: 0 }
                                        }
                                    },
                                    vertex_data: match vertex_data {
                                        DeviceOrHostAddress::DeviceAddress(device_address) => {
                                            vk::DeviceOrHostAddressConstKHR { device_address }
                                        }
                                        DeviceOrHostAddress::HostAddress => todo!(),
                                    },
                                    vertex_format,
                                    vertex_stride,
                                    ..Default::default()
                                },
                            },
                        ),
                    };

                    tls.geometries.push(vk::AccelerationStructureGeometryKHR {
                        flags,
                        geometry_type,
                        geometry,
                        ..Default::default()
                    });
                    tls.max_primitive_counts.push(info.max_primitive_count);
                }

                let info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                    .ty(build_info.ty)
                    .flags(build_info.flags)
                    .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                    .geometries(&tls.geometries)
                    .dst_acceleration_structure(*self.bindings[accel_struct_node])
                    .scratch_data(vk::DeviceOrHostAddressKHR {
                        device_address: Buffer::device_address(&self.bindings[scratch_buf_node]),
                    });

                self.device
                    .accel_struct_ext
                    .as_ref()
                    .unwrap()
                    .cmd_build_acceleration_structures(
                        self.cmd_buf,
                        from_ref(&info),
                        from_ref(&build_ranges),
                    );
            });
        }
    }
}

pub trait Access {
    const DEFAULT_READ: AccessType;
    const DEFAULT_WRITE: AccessType;
}

impl Access for ComputePipeline {
    const DEFAULT_READ: AccessType = AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::ComputeShaderWrite;
}

impl Access for GraphicPipeline {
    const DEFAULT_READ: AccessType = AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::AnyShaderWrite;
}

impl Access for RayTracePipeline {
    const DEFAULT_READ: AccessType =
        AccessType::RayTracingShaderReadSampledImageOrUniformTexelBuffer;
    const DEFAULT_WRITE: AccessType = AccessType::AnyShaderWrite;
}

macro_rules! bind {
    ($name:ident) => {
        paste::paste! {
            impl<'a> Bind<PassRef<'a>, PipelinePassRef<'a, [<$name Pipeline>]>> for &'a Arc<[<$name Pipeline>]> {
                // TODO: Allow binding as explicit secondary command buffers? like with compute/raytrace stuff
                fn bind(self, mut pass: PassRef<'a>) -> PipelinePassRef<'_, [<$name Pipeline>]> {
                    let pass_ref = pass.as_mut();
                    if pass_ref.execs.last().unwrap().pipeline.is_some() {
                        // Binding from PipelinePass -> PipelinePass (changing shaders)
                        pass_ref.execs.push(Default::default());
                    }

                    pass_ref.execs.last_mut().unwrap().pipeline = Some(ExecutionPipeline::$name(Arc::clone(self)));

                    PipelinePassRef {
                        __: PhantomData,
                        pass,
                    }
                }
            }

            impl ExecutionPipeline {
                #[allow(unused)]
                pub(super) fn [<is_ $name:snake>](&self) -> bool {
                    matches!(self, Self::$name(_))
                }

                #[allow(unused)]
                pub(super) fn [<unwrap_ $name:snake>](&self) -> &Arc<[<$name Pipeline>]> {
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

pub struct Bindings<'a> {
    pub(super) exec: &'a Execution,
    pub(super) graph: &'a RenderGraph,
}

impl<'a> Bindings<'a> {
    fn binding_ref(&self, node_idx: usize) -> &Binding {
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
            impl<'a> Index<[<$name Node>]> for Bindings<'a>
            {
                type Output = $handle;

                fn index(&self, node: [<$name Node>]) -> &Self::Output {
                    &*self.binding_ref(node.idx).[<as_ $name:snake>]().unwrap().item
                }
            }
        }
    };
}

// Allow indexing the Bindings data during command execution:
// (This gets you access to the driver images or other resources)
index!(AccelerationStructure, AccelerationStructure);
index!(AccelerationStructureLease, AccelerationStructure);
index!(Buffer, Buffer);
index!(BufferLease, Buffer);
index!(Image, Image);
index!(ImageLease, Image);
index!(SwapchainImage, Image);

impl<'a> Index<AnyAccelerationStructureNode> for Bindings<'a> {
    type Output = AccelerationStructure;

    fn index(&self, node: AnyAccelerationStructureNode) -> &Self::Output {
        let node_idx = match node {
            AnyAccelerationStructureNode::AccelerationStructure(node) => node.idx,
            AnyAccelerationStructureNode::AccelerationStructureLease(node) => node.idx,
        };
        let binding = self.binding_ref(node_idx);

        match node {
            AnyAccelerationStructureNode::AccelerationStructure(_) => {
                &binding.as_acceleration_structure().unwrap().item
            }
            AnyAccelerationStructureNode::AccelerationStructureLease(_) => {
                &binding.as_acceleration_structure_lease().unwrap().item
            }
        }
    }
}

impl<'a> Index<AnyBufferNode> for Bindings<'a> {
    type Output = Buffer;

    fn index(&self, node: AnyBufferNode) -> &Self::Output {
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

impl<'a> Index<AnyImageNode> for Bindings<'a> {
    type Output = Image;

    fn index(&self, node: AnyImageNode) -> &Self::Output {
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

pub struct Compute<'a> {
    bindings: Bindings<'a>,
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
    pipeline: Arc<ComputePipeline>,
}

impl<'a> Compute<'a> {
    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) -> &Self {
        unsafe {
            self.device
                .cmd_dispatch(self.cmd_buf, group_count_x, group_count_y, group_count_z);
        }

        self
    }

    pub fn dispatch_base(
        &self,
        base_group_x: u32,
        base_group_y: u32,
        base_group_z: u32,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
    ) -> &Self {
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
        &self,
        args_buf: impl Into<AnyBufferNode>,
        args_offset: vk::DeviceSize,
    ) -> &Self {
        let args_buf = args_buf.into();

        unsafe {
            self.device
                .cmd_dispatch_indirect(self.cmd_buf, *self.bindings[args_buf], args_offset);
        }

        self
    }

    pub fn push_constants(&self, data: &[u8]) -> &Self {
        self.push_constants_offset(0, data)
    }

    pub fn push_constants_offset(&self, offset: u32, data: &[u8]) -> &Self {
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

pub struct Draw<'a> {
    bindings: Bindings<'a>,
    buffers: &'a RefCell<Vec<vk::Buffer>>,
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
    offsets: &'a RefCell<Vec<vk::DeviceSize>>,
    pipeline: Arc<GraphicPipeline>,
    rects: &'a RefCell<Vec<vk::Rect2D>>,
    viewports: &'a RefCell<Vec<vk::Viewport>>,
}

impl<'a> Draw<'a> {
    pub fn bind_index_buffer(
        &self,
        buffer: impl Into<AnyBufferNode>,
        index_ty: vk::IndexType,
    ) -> &Self {
        self.bind_index_buffer_offset(buffer, index_ty, 0)
    }

    pub fn bind_index_buffer_offset(
        &self,
        buffer: impl Into<AnyBufferNode>,
        index_ty: vk::IndexType,
        offset: vk::DeviceSize,
    ) -> &Self {
        let buffer = buffer.into();

        unsafe {
            self.device.cmd_bind_index_buffer(
                self.cmd_buf,
                *self.bindings[buffer],
                offset,
                index_ty,
            );
        }

        self
    }

    pub fn bind_vertex_buffer(&self, buffer: impl Into<AnyBufferNode>) -> &Self {
        self.bind_vertex_buffer_offset(buffer, 0)
    }

    pub fn bind_vertex_buffer_offset(
        &self,
        buffer: impl Into<AnyBufferNode>,
        offset: vk::DeviceSize,
    ) -> &Self {
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
        &self,
        first_binding: u32,
        buffers: impl IntoIterator<Item = (B, vk::DeviceSize)>,
    ) -> &Self
    where
        B: Into<AnyBufferNode>,
    {
        let mut buffers_vec = self.buffers.borrow_mut();
        buffers_vec.clear();

        let mut offsets_vec = self.offsets.borrow_mut();
        offsets_vec.clear();

        for (buffer, offset) in buffers {
            let buffer = buffer.into();

            buffers_vec.push(*self.bindings[buffer]);
            offsets_vec.push(offset);
        }

        unsafe {
            self.device.cmd_bind_vertex_buffers(
                self.cmd_buf,
                first_binding,
                buffers_vec.as_slice(),
                offsets_vec.as_slice(),
            );
        }

        self
    }

    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> &Self {
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
        &self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) -> &Self {
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
        &self,
        buffer: impl Into<AnyBufferNode>,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) -> &Self {
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
        &self,
        buffer: impl Into<AnyBufferNode>,
        offset: vk::DeviceSize,
        count_buf: impl Into<AnyBufferNode>,
        count_buf_offset: vk::DeviceSize,
        max_draw_count: u32,
        stride: u32,
    ) -> &Self {
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
        &self,
        buffer: impl Into<AnyBufferNode>,
        offset: vk::DeviceSize,
        draw_count: u32,
        stride: u32,
    ) -> &Self {
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
        &self,
        buffer: impl Into<AnyBufferNode>,
        offset: vk::DeviceSize,
        count_buf: impl Into<AnyBufferNode>,
        count_buf_offset: vk::DeviceSize,
        max_draw_count: u32,
        stride: u32,
    ) -> &Self {
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

    pub fn push_constants(&self, data: &[u8]) -> &Self {
        self.push_constants_offset(0, data)
    }

    pub fn push_constants_offset(&self, offset: u32, data: &[u8]) -> &Self {
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

    pub fn set_scissor(&self, x: i32, y: i32, width: u32, height: u32) -> &Self {
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
        &self,
        first_scissor: u32,
        scissors: impl IntoIterator<Item = S>,
    ) -> &Self
    where
        S: Into<vk::Rect2D>,
    {
        let mut rects_vec = self.rects.borrow_mut();
        rects_vec.clear();

        for scissor in scissors {
            rects_vec.push(scissor.into());
        }

        unsafe {
            self.device
                .cmd_set_scissor(self.cmd_buf, first_scissor, rects_vec.as_slice());
        }

        self
    }

    pub fn set_viewport(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        depth: Range<f32>,
    ) -> &Self {
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
        &self,
        first_viewport: u32,
        viewports: impl IntoIterator<Item = V>,
    ) -> &Self
    where
        V: Into<vk::Viewport>,
    {
        let mut viewports_vec = self.viewports.borrow_mut();
        viewports_vec.clear();

        for viewport in viewports {
            viewports_vec.push(viewport.into());
        }

        unsafe {
            self.device
                .cmd_set_viewport(self.cmd_buf, first_viewport, viewports_vec.as_slice());
        }

        self
    }
}

pub struct PassRef<'a> {
    pub(super) exec_idx: usize,
    pub(super) graph: &'a mut RenderGraph,
    pub(super) pass_idx: usize,
}

impl<'a> PassRef<'a> {
    pub(super) fn new(graph: &'a mut RenderGraph, name: String) -> PassRef<'a> {
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
    pub fn access_node(mut self, node: impl Node + Information, access: AccessType) -> Self {
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
        N: View,
    {
        self.push_node_access(node, access, Some(subresource.into().into()));
        self
    }

    fn as_mut(&mut self) -> &mut Pass {
        &mut self.graph.passes[self.pass_idx]
    }

    fn as_ref(&self) -> &Pass {
        &self.graph.passes[self.pass_idx]
    }

    fn assert_bound_graph_node(&self, node: impl Node) {
        let idx = node.index();

        assert!(self.graph.bindings[idx].is_bound());
    }

    pub fn bind_pipeline<B>(self, binding: B) -> <B as Edge<Self>>::Result
    where
        B: Edge<Self>,
        B: Bind<Self, <B as Edge<Self>>::Result>,
    {
        binding.bind(self)
    }

    fn push_execute(
        &mut self,
        func: impl FnOnce(&Device, vk::CommandBuffer, Bindings<'_>) + Send + 'static,
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
        node: impl Node,
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

    pub fn read_node(self, node: impl Node + Information) -> Self {
        let access = AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer;
        self.access_node(node, access)
    }

    pub fn record_acceleration(
        mut self,
        func: impl FnOnce(Acceleration<'_>) + Send + 'static,
    ) -> Self {
        self.push_execute(move |device, cmd_buf, bindings| {
            func(Acceleration {
                bindings,
                cmd_buf,
                device,
            });
        });

        self
    }

    pub fn record_cmd_buf(
        mut self,
        func: impl FnOnce(&Device, vk::CommandBuffer, Bindings<'_>) + Send + 'static,
    ) -> Self {
        self.push_execute(func);
        self
    }

    pub fn submit_pass(self) -> &'a mut RenderGraph {
        // If nothing was done in this pass we can just ignore it
        if self.exec_idx == 0 {
            self.graph.passes.pop();
        }

        self.graph
    }

    pub fn write_node(self, node: impl Node + Information) -> Self {
        let access = AccessType::AnyShaderWrite;
        self.access_node(node, access)
    }
}

pub struct PipelinePassRef<'a, T>
where
    T: Access,
{
    __: PhantomData<T>,
    pass: PassRef<'a>,
}

impl<'a, T> PipelinePassRef<'a, T>
where
    T: Access,
{
    pub fn access_descriptor<N>(
        self,
        descriptor: impl Into<Descriptor>,
        node: N,
        access: AccessType,
    ) -> Self
    where
        N: Information,
        N: View,
        ViewType: From<<N as View>::Information>,
        <N as View>::Information: From<<N as Information>::Info>,
        <N as View>::Subresource: From<<N as View>::Information>,
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
        N: View,
        <N as View>::Information: Into<ViewType>,
        <N as View>::Subresource: From<<N as View>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View>::Subresource::from(<N as View>::Information::clone(&view_info));

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
        N: View,
        <N as View>::Information: Into<ViewType>,
    {
        self.pass
            .push_node_access(node, access, Some(subresource.into().into()));
        self.push_node_view_bind(node, view_info.into(), descriptor.into());

        self
    }

    pub fn access_node(mut self, node: impl Node + Information, access: AccessType) -> Self {
        self.pass.assert_bound_graph_node(node);

        let idx = node.index();
        let binding = &self.pass.graph.bindings[idx];

        let mut node_access_range = None;
        if let Some(buf) = binding.as_driver_buffer() {
            node_access_range = Some(Subresource::Buffer((0..buf.info.size).into()));
        } else if let Some(image) = binding.as_driver_image() {
            node_access_range = Some(Subresource::Image(image.info.default_view_info().into()))
        }

        self.pass.push_node_access(node, access, node_access_range);
        self
    }

    pub fn access_node_subrange<N>(
        mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View,
    {
        self.pass
            .push_node_access(node, access, Some(subresource.into().into()));
        self
    }

    fn push_node_view_bind(
        &mut self,
        node: impl Node,
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
            "descriptor {binding:?} has already been bound"
        );
    }

    pub fn read_descriptor<N>(self, descriptor: impl Into<Descriptor>, node: N) -> Self
    where
        N: Information,
        N: View,
        ViewType: From<<N as View>::Information>,
        <N as View>::Information: From<<N as Information>::Info>,
        <N as View>::Subresource: From<<N as View>::Information>,
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
        N: View,
        <N as View>::Information: Into<ViewType>,
        <N as View>::Subresource: From<<N as View>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View>::Subresource::from(<N as View>::Information::clone(&view_info));

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
        N: View,
        <N as View>::Information: Into<ViewType>,
    {
        let access = <T as Access>::DEFAULT_READ;
        self.access_descriptor_subrange(descriptor, node, access, view_info, subresource)
    }

    pub fn read_node(self, node: impl Node + Information) -> Self {
        let access = <T as Access>::DEFAULT_READ;
        self.access_node(node, access)
    }

    pub fn read_node_subrange<N>(self, node: N, subresource: impl Into<N::Subresource>) -> Self
    where
        N: View,
    {
        let access = <T as Access>::DEFAULT_READ;
        self.access_node_subrange(node, access, subresource)
    }

    pub fn submit_pass(self) -> &'a mut RenderGraph {
        self.pass.submit_pass()
    }

    pub fn write_descriptor<N>(self, descriptor: impl Into<Descriptor>, node: N) -> Self
    where
        N: Information,
        N: View,
        <N as View>::Information: Into<ViewType>,
        <N as View>::Information: From<<N as Information>::Info>,
        <N as View>::Subresource: From<<N as View>::Information>,
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
        N: View,
        <N as View>::Information: Into<ViewType>,
        <N as View>::Subresource: From<<N as View>::Information>,
    {
        let view_info = view_info.into();
        let subresource =
            <N as View>::Subresource::from(<N as View>::Information::clone(&view_info));

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
        N: View,
        <N as View>::Information: Into<ViewType>,
    {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_descriptor_subrange(descriptor, node, access, view_info, subresource)
    }

    pub fn write_node(self, node: impl Node + Information) -> Self {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_node(node, access)
    }

    pub fn write_node_subrange<N>(self, node: N, subresource: impl Into<N::Subresource>) -> Self
    where
        N: View,
    {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_node_subrange(node, access, subresource)
    }
}

impl<'a> PipelinePassRef<'a, ComputePipeline> {
    pub fn record_compute(mut self, func: impl FnOnce(Compute<'_>) + Send + 'static) -> Self {
        let pipeline = Arc::clone(
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
            func(Compute {
                bindings,
                cmd_buf,
                device,
                pipeline,
            });
        });

        self
    }
}

impl<'a> PipelinePassRef<'a, GraphicPipeline> {
    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
    ///
    /// The image is
    pub fn attach_color(self, attachment: AttachmentIndex, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.attach_color_as(attachment, image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
    pub fn attach_color_as(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode>,
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
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.attach_depth_stencil_as(attachment, image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` and `VK_ATTACHMENT_STORE_OP_STORE` for the render
    /// pass attachment.
    pub fn attach_depth_stencil_as(
        self,
        attachment: AttachmentIndex,
        image: impl Into<AnyImageNode>,
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

        debug_assert!(
            !exec
                .pipeline
                .as_ref()
                .unwrap()
                .unwrap_graphic()
                .input_attachments
                .contains(&attachment),
            "cleared attachment uses subpass input"
        );

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

        debug_assert!(
            !exec
                .pipeline
                .as_ref()
                .unwrap()
                .unwrap_graphic()
                .input_attachments
                .contains(&attachment),
            "cleared attachment uses subpass input"
        );

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
        let image_info = &self.pass.graph.bindings[node_idx].image_info().unwrap();

        (image_info.fmt, image_info.sample_count)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
    pub fn load_color(self, attachment: AttachmentIndex, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            debug_assert!(
                !exec
                    .pipeline
                    .as_ref()
                    .unwrap()
                    .unwrap_graphic()
                    .input_attachments
                    .contains(&attachment),
                "attachment uses subpass input"
            );

            assert!(
                exec.loads.insert_color(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous load"
            );

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

                assert!(
                    exec.stores
                        .attached
                        .get(attachment as usize)
                        .map(|stored_attachment| Attachment::are_compatible(
                            *stored_attachment,
                            Some(color_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous store"
                );
                assert!(
                    exec.resolves
                        .attached
                        .get(attachment as usize)
                        .map(|resolved_attachment| Attachment::are_compatible(
                            *resolved_attachment,
                            Some(color_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous resolve"
                );
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
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            debug_assert!(
                !exec
                    .pipeline
                    .as_ref()
                    .unwrap()
                    .unwrap_graphic()
                    .input_attachments
                    .contains(&attachment),
                "attachment uses subpass input"
            );

            assert!(
                exec.loads.set_depth_stencil(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous load"
            );

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, loaded_attachment) = exec.loads.depth_stencil().unwrap();

                assert!(
                    exec.stores
                        .depth_stencil()
                        .map(
                            |(attachment_idx, stored_attachment)| attachment == attachment_idx
                                && Attachment::are_identical(stored_attachment, loaded_attachment)
                        )
                        .unwrap_or(true),
                    "attachment incompatible with previous store"
                );
                assert!(
                    exec.resolves
                        .depth_stencil()
                        .map(
                            |(attachment_idx, resolved_attachment)| attachment == attachment_idx
                                && Attachment::are_identical(
                                    resolved_attachment,
                                    loaded_attachment
                                )
                        )
                        .unwrap_or(true),
                    "attachment incompatible with previous resolve"
                );
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
    pub fn record_subpass(mut self, func: impl FnOnce(Draw<'_>) + Send + 'static) -> Self {
        let pipeline = {
            let exec = self.pass.as_ref().execs.last().unwrap();
            let pipeline = exec.pipeline.as_ref().unwrap().unwrap_graphic();

            #[cfg(debug_assertions)]
            for attachment in exec.attached_written() {
                assert!(
                    pipeline.write_attachments.contains(&attachment),
                    "attachment {attachment} not written by shader"
                );
            }

            Arc::clone(pipeline)
        };

        self.pass.push_execute(move |device, cmd_buf, bindings| {
            #[derive(Default)]
            struct Tls {
                buffers: RefCell<Vec<vk::Buffer>>,
                offsets: RefCell<Vec<vk::DeviceSize>>,
                rects: RefCell<Vec<vk::Rect2D>>,
                viewports: RefCell<Vec<vk::Viewport>>,
            }

            thread_local! {
                static TLS: Tls = Default::default();
            }

            TLS.with(
                |Tls {
                     buffers,
                     offsets,
                     rects,
                     viewports,
                 }| {
                    func(Draw {
                        bindings,
                        buffers,
                        cmd_buf,
                        device,
                        offsets,
                        pipeline,
                        rects,
                        viewports,
                    });
                },
            );
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
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            assert!(
                exec.resolves.insert_color(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous resolve"
            );

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

                assert!(
                    exec.clears.contains_key(&attachment)
                        || exec
                            .loads
                            .attached
                            .get(attachment as usize)
                            .map(|loaded_attachment| Attachment::are_compatible(
                                *loaded_attachment,
                                Some(resolved_attachment)
                            ))
                            .unwrap_or(true)
                        || exec
                            .pipeline
                            .as_ref()
                            .unwrap()
                            .unwrap_graphic()
                            .input_attachments
                            .contains(&attachment),
                    "attachment resolved without clear, or compatible load, or subpass input"
                );
                assert!(
                    exec.stores
                        .attached
                        .get(attachment as usize)
                        .map(|&stored_attachment| Attachment::are_compatible(
                            stored_attachment,
                            Some(resolved_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous store"
                );
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
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            assert!(
                exec.resolves.set_depth_stencil(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous store"
            );

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, resolved_attachment) = exec.resolves.depth_stencil().unwrap();

                assert!(
                    exec.clears.contains_key(&attachment)
                        || exec
                            .loads
                            .depth_stencil()
                            .map(
                                |(attachment_idx, loaded_attachment)| attachment == attachment_idx
                                    && Attachment::are_identical(
                                        loaded_attachment,
                                        resolved_attachment
                                    )
                            )
                            .unwrap_or(true)
                        || exec
                            .pipeline
                            .as_ref()
                            .unwrap()
                            .unwrap_graphic()
                            .input_attachments
                            .contains(&attachment),
                    "attachment resolved without clear, or compatible load, or subpass input"
                );
                assert!(
                    exec.stores
                        .attached
                        .get(attachment as usize)
                        .map(|&stored_attachment| Attachment::are_compatible(
                            stored_attachment,
                            Some(resolved_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous store"
                );
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
    pub fn store_color(self, attachment: AttachmentIndex, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            assert!(
                exec.stores.insert_color(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous store"
            );

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

                assert!(
                    exec.clears.contains_key(&attachment)
                        || exec
                            .loads
                            .attached
                            .get(attachment as usize)
                            .map(|loaded_attachment| Attachment::are_compatible(
                                *loaded_attachment,
                                Some(stored_attachment)
                            ))
                            .unwrap_or(true)
                        || exec
                            .pipeline
                            .as_ref()
                            .unwrap()
                            .unwrap_graphic()
                            .input_attachments
                            .contains(&attachment),
                    "attachment stored without clear, compatible load, or subpass input"
                );
                assert!(
                    exec.resolves
                        .attached
                        .get(attachment as usize)
                        .map(|&resolved_attachment| Attachment::are_compatible(
                            resolved_attachment,
                            Some(stored_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous resolve"
                );
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
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
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
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        {
            let pass = self.pass.as_mut();
            let exec = pass.execs.last_mut().unwrap();

            assert!(
                exec.stores.set_depth_stencil(
                    attachment,
                    image_view_info.aspect_mask,
                    image_view_info.fmt,
                    sample_count,
                    node_idx,
                ),
                "attachment incompatible with previous store"
            );

            #[cfg(debug_assertions)]
            {
                // Unwrap the attachment we inserted above
                let (_, stored_attachment) = exec.stores.depth_stencil().unwrap();

                assert!(
                    exec.clears.contains_key(&attachment)
                        || exec
                            .loads
                            .depth_stencil()
                            .map(|(attachment_idx, loaded_attachment)| {
                                attachment == attachment_idx
                                    && Attachment::are_identical(
                                        loaded_attachment,
                                        stored_attachment,
                                    )
                            })
                            .unwrap_or(true)
                        || exec
                            .pipeline
                            .as_ref()
                            .unwrap()
                            .unwrap_graphic()
                            .input_attachments
                            .contains(&attachment),
                    "attachment stored without clear, or comptaible load, or subpass input"
                );
                assert!(
                    exec.resolves
                        .attached
                        .get(attachment as usize)
                        .map(|&resolved_attachment| Attachment::are_compatible(
                            resolved_attachment,
                            Some(stored_attachment)
                        ))
                        .unwrap_or(true),
                    "attachment incompatible with previous resolve"
                );
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

impl<'a> PipelinePassRef<'a, RayTracePipeline> {
    pub fn record_ray_trace(mut self, func: impl FnOnce(RayTrace<'_>) + Send + 'static) -> Self {
        self.pass.push_execute(move |device, cmd_buf, _bindings| {
            func(RayTrace { cmd_buf, device });
        });

        self
    }
}

pub struct RayTrace<'a> {
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
}

impl<'a> RayTrace<'a> {
    // TODO: If the rayTraversalPrimitiveCulling or rayQuery features are enabled, the SkipTrianglesKHR and SkipAABBsKHR ray flags can be specified when tracing a ray. SkipTrianglesKHR and SkipAABBsKHR are mutually exclusive.

    #[allow(clippy::too_many_arguments)]
    pub fn trace_rays(
        &self,
        raygen_shader_binding_tables: &vk::StridedDeviceAddressRegionKHR,
        miss_shader_binding_tables: &vk::StridedDeviceAddressRegionKHR,
        hit_shader_binding_tables: &vk::StridedDeviceAddressRegionKHR,
        callable_shader_binding_tables: &vk::StridedDeviceAddressRegionKHR,
        width: u32,
        height: u32,
        depth: u32,
    ) -> &Self {
        unsafe {
            self.device
                .ray_tracing_pipeline_ext
                .as_ref()
                .unwrap()
                .cmd_trace_rays(
                    self.cmd_buf,
                    raygen_shader_binding_tables,
                    miss_shader_binding_tables,
                    hit_shader_binding_tables,
                    callable_shader_binding_tables,
                    width,
                    height,
                    depth,
                );
        }

        self
    }

    pub fn trace_rays_indirect(
        &self,
        // _tlas: RayTraceAccelerationNode,
        _args_buf: BufferNode,
        _args_buf_offset: vk::DeviceSize,
    ) -> &Self {
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
