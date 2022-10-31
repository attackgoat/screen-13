//! Strongly-typed rendering commands.

use {
    super::{
        AccelerationStructureLeaseNode, AccelerationStructureNode, AnyAccelerationStructureNode,
        AnyBufferNode, AnyImageNode, Area, Attachment, Bind, Binding, BufferLeaseNode, BufferNode,
        ClearColorValue, Edge, Execution, ExecutionFunction, ExecutionPipeline, ImageLeaseNode,
        ImageNode, Information, Node, NodeIndex, Pass, RenderGraph, SampleCount,
        SwapchainImageNode,
    },
    crate::driver::{
        accel_struct::{AccelerationStructure, AccelerationStructureGeometryInfo},
        buffer::{Buffer, BufferSubresource},
        compute::ComputePipeline,
        graphic::{DepthStencilMode, GraphicPipeline},
        image::{Image, ImageSubresource, ImageViewInfo},
        ray_trace::RayTracePipeline,
        Device,
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

/// Alias for the index of a framebuffer attachment.
pub type AttachmentIndex = u32;

/// Alias for the binding index of a shader descriptor.
pub type BindingIndex = u32;

/// Alias for the binding offset of a shader descriptor array element.
pub type BindingOffset = u32;

/// Alias for the descriptor set index of a shader descriptor.
pub type DescriptorSetIndex = u32;

/// Recording interface for acceleration structure commands.
///
/// This structure provides a strongly-typed set of methods which allow acceleration structures to
/// be built and updated. An instance of `Acceleration` is provided to the closure parameter of
/// [`PassRef::record_acceleration`].
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureInfo};
/// # use screen_13::driver::{Device, DriverConfig, DriverError};
/// # use screen_13::graph::RenderGraph;
/// # use screen_13::driver::shader::Shader;
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let mut my_graph = RenderGraph::new();
/// # let info = AccelerationStructureInfo::new_blas(1);
/// my_graph.begin_pass("my acceleration pass")
///         .record_acceleration(move |acceleration, bindings| {
///             // During this closure we have access to the acceleration methods!
///         });
/// # Ok(()) }
/// ```
pub struct Acceleration<'a> {
    bindings: Bindings<'a>,
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
}

impl<'a> Acceleration<'a> {
    /// Build an acceleration structure.
    ///
    /// Requires a scratch buffer which was created with the following requirements:
    ///
    /// - Flags must include [`vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS`]
    /// - Size must be equal to or greater than the `build_size` value returned by
    ///   [`AccelerationStructure::size_of`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureGeometry, AccelerationStructureGeometryData, AccelerationStructureGeometryInfo, AccelerationStructureInfo, DeviceOrHostAddress};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # use screen_13::driver::shader::Shader;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let info = AccelerationStructureInfo::new_blas(1);
    /// # let blas_accel_struct = AccelerationStructure::create(&device, info)?;
    /// # let blas_node = my_graph.bind_node(blas_accel_struct);
    /// # let scratch_buf_info = BufferInfo::new(8, vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);
    /// # let scratch_buf = Buffer::create(&device, scratch_buf_info)?;
    /// # let scratch_buf = my_graph.bind_node(scratch_buf);
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::INDEX_BUFFER);
    /// # let my_idx_buf = Buffer::create(&device, buf_info)?;
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::VERTEX_BUFFER);
    /// # let my_vtx_buf = Buffer::create(&device, buf_info)?;
    /// # let index_node = my_graph.bind_node(my_idx_buf);
    /// # let vertex_node = my_graph.bind_node(my_vtx_buf);
    /// my_graph.begin_pass("my acceleration pass")
    ///         .read_node(index_node)
    ///         .read_node(vertex_node)
    ///         .write_node(blas_node)
    ///         .write_node(scratch_buf)
    ///         .record_acceleration(move |acceleration, bindings| {
    ///             let info = AccelerationStructureGeometryInfo {
    ///                 ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
    ///                 flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
    ///                 geometries: vec![AccelerationStructureGeometry {
    ///                     max_primitive_count: 64,
    ///                     flags: vk::GeometryFlagsKHR::OPAQUE,
    ///                     geometry: AccelerationStructureGeometryData::Triangles {
    ///                         index_data: DeviceOrHostAddress::DeviceAddress(
    ///                             Buffer::device_address(&bindings[index_node])
    ///                         ),
    ///                         index_type: vk::IndexType::UINT32,
    ///                         max_vertex: 42,
    ///                         transform_data: None,
    ///                         vertex_data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(
    ///                             &bindings[vertex_node],
    ///                         )),
    ///                         vertex_format: vk::Format::R32G32B32_SFLOAT,
    ///                         vertex_stride: 12,
    ///                     },
    ///                 }],
    ///             };
    ///             let ranges = vk::AccelerationStructureBuildRangeInfoKHR {
    ///                 first_vertex: 0,
    ///                 primitive_count: 1,
    ///                 primitive_offset: 0,
    ///                 transform_offset: 0,
    ///             };
    ///
    ///             acceleration.build_structure(blas_node, scratch_buf, &info, &[ranges]);
    ///         });
    /// # Ok(()) }
    /// ```
    pub fn build_structure(
        &self,
        accel_struct_node: impl Into<AnyAccelerationStructureNode>,
        scratch_buf_node: impl Into<AnyBufferNode>,
        build_info: &AccelerationStructureGeometryInfo,
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
                    tls.geometries.push(info.into_vk());
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

    /// Update an acceleration structure.
    ///
    /// Requires a scratch buffer which was created with the following requirements:
    ///
    /// - Flags must include [`vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS`]
    /// - Size must be equal to or greater than the `update_size` value returned by
    ///   [`AccelerationStructure::size_of`].
    pub fn update_structure(
        &self,
        src_accel_node: impl Into<AnyAccelerationStructureNode>,
        dst_accel_node: impl Into<AnyAccelerationStructureNode>,
        scratch_buf_node: impl Into<AnyBufferNode>,
        build_info: &AccelerationStructureGeometryInfo,
        build_ranges: &[vk::AccelerationStructureBuildRangeInfoKHR],
    ) {
        use std::slice::from_ref;

        let src_accel_node = src_accel_node.into();
        let dst_accel_node = dst_accel_node.into();
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
                    tls.geometries.push(info.into_vk());
                    tls.max_primitive_counts.push(info.max_primitive_count);
                }

                let info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                    .ty(build_info.ty)
                    .flags(build_info.flags)
                    .mode(vk::BuildAccelerationStructureModeKHR::UPDATE)
                    .geometries(&tls.geometries)
                    .dst_acceleration_structure(*self.bindings[dst_accel_node])
                    .src_acceleration_structure(*self.bindings[src_accel_node])
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

/// Associated type trait which enables default values for read and write methods.
pub trait Access {
    /// The default `AccessType` for read operations, if not specified explicitly.
    const DEFAULT_READ: AccessType;

    /// The default `AccessType` for write operations, if not specified explicitly.
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

/// An indexable structure will provides access to Vulkan smart-pointer resources inside a record
/// closure.
///
/// This type is available while recording commands in the following closures:
///
/// - [`PassRef::record_acceleration`] for building and updating acceleration structures
/// - [`PassRef::record_cmd_buf`] for general command streams
/// - [`PipelinePassRef::record_compute`] for dispatched compute operations
/// - [`PipelinePassRef::record_subpass`] for raster drawing operations, such as triangles streams
/// - [`PipelinePassRef::record_ray_trace`] for ray-traced operations
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{Device, DriverConfig, DriverError};
/// # use screen_13::driver::image::{Image, ImageInfo};
/// # use screen_13::graph::RenderGraph;
/// # use screen_13::graph::node::ImageNode;
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
/// # let image = Image::create(&device, info)?;
/// # let mut my_graph = RenderGraph::new();
/// # let my_image_node = my_graph.bind_node(image);
/// my_graph.begin_pass("custom vulkan commands")
///         .record_cmd_buf(move |device, cmd_buf, bindings| {
///             let my_image = &bindings[my_image_node];
///
///             assert_ne!(**my_image, vk::Image::null());
///             assert_eq!(my_image.info.width, 32);
///         });
/// # Ok(()) }
/// ```
#[derive(Clone, Copy, Debug)]
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
                    &*self.binding_ref(node.idx).[<as_ $name:snake>]().unwrap()
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
                binding.as_acceleration_structure().unwrap()
            }
            AnyAccelerationStructureNode::AccelerationStructureLease(_) => {
                binding.as_acceleration_structure_lease().unwrap()
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
            AnyBufferNode::Buffer(_) => binding.as_buffer().unwrap(),
            AnyBufferNode::BufferLease(_) => binding.as_buffer_lease().unwrap(),
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
            AnyImageNode::Image(_) => binding.as_image().unwrap(),
            AnyImageNode::ImageLease(_) => binding.as_image_lease().unwrap(),
            AnyImageNode::SwapchainImage(_) => binding.as_swapchain_image().unwrap(),
        }
    }
}

/// Recording interface for computing commands.
///
/// This structure provides a strongly-typed set of methods which allow compute shader code to be
/// executed. An instance of `Compute` is provided to the closure parameter of
/// [`PipelinePassRef::record_compute`] which may be accessed by binding a [`ComputePipeline`] to a
/// render pass.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{Device, DriverConfig, DriverError};
/// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
/// # use screen_13::graph::RenderGraph;
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let shader = [0u8; 1];
/// # let info = ComputePipelineInfo::new(shader.as_slice());
/// # let my_compute_pipeline = Arc::new(ComputePipeline::create(&device, info)?);
/// # let mut my_graph = RenderGraph::new();
/// my_graph.begin_pass("my compute pass")
///         .bind_pipeline(&my_compute_pipeline)
///         .record_compute(move |compute, bindings| {
///             // During this closure we have access to the compute methods!
///         });
/// # Ok(()) }
/// ```
pub struct Compute<'a> {
    bindings: Bindings<'a>,
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
    pipeline: Arc<ComputePipeline>,
}

impl<'a> Compute<'a> {
    /// [Dispatch] compute work items.
    ///
    /// When the command is executed, a global workgroup consisting of
    /// `group_count_x × group_count_y × group_count_z` local workgroups is assembled.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 450
    ///
    /// layout(set = 0, binding = 0, std430) restrict writeonly buffer MyBufer {
    ///     uint my_buf[];
    /// };
    ///
    /// void main()
    /// {
    ///     // TODO
    /// }
    /// # "#, comp);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::STORAGE_BUFFER);
    /// # let my_buf = Buffer::create(&device, buf_info)?;
    /// # let shader = [0u8; 1];
    /// # let pipeline_info = ComputePipelineInfo::new(shader.as_slice());
    /// # let my_compute_pipeline = Arc::new(ComputePipeline::create(&device, pipeline_info)?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let my_buf_node = my_graph.bind_node(my_buf);
    /// my_graph.begin_pass("fill my_buf_node with data")
    ///         .bind_pipeline(&my_compute_pipeline)
    ///         .write_descriptor(0, my_buf_node)
    ///         .record_compute(move |compute, bindings| {
    ///             compute.dispatch(128, 64, 32);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [Dispatch]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/vkCmdDispatch.html
    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) -> &Self {
        unsafe {
            self.device
                .cmd_dispatch(self.cmd_buf, group_count_x, group_count_y, group_count_z);
        }

        self
    }

    /// [Dispatch] compute work items with non-zero base values for the workgroup IDs.
    ///
    /// When the command is executed, a global workgroup consisting of
    /// `group_count_x × group_count_y × group_count_z` local workgroups is assembled, with
    /// WorkgroupId values ranging from `[base_group*, base_group* + group_count*)` in each
    /// component.
    ///
    /// [`Compute::dispatch`] is equivalent to
    /// `dispatch_base(0, 0, 0, group_count_x, group_count_y, group_count_z)`.
    ///
    /// [Dispatch]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/vkCmdDispatchBase.html
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

    /// Dispatch compute work items with indirect parameters.
    ///
    /// `dispatch_indirect` behaves similarly to [`Compute::dispatch`] except that the parameters
    /// are read by the device from `args_buf` during execution. The parameters of the dispatch are
    /// encoded in a [`vk::DispatchIndirectCommand`] structure taken from `args_buf` starting at
    /// `args_offset`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use std::mem::size_of;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::STORAGE_BUFFER);
    /// # let my_buf = Buffer::create(&device, buf_info)?;
    /// # let shader = [0u8; 1];
    /// # let pipeline_info = ComputePipelineInfo::new(shader.as_slice());
    /// # let my_compute_pipeline = Arc::new(ComputePipeline::create(&device, pipeline_info)?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let my_buf_node = my_graph.bind_node(my_buf);
    /// const CMD_SIZE: usize = size_of::<vk::DispatchIndirectCommand>();
    ///
    /// let cmd = vk::DispatchIndirectCommand {
    ///     x: 1,
    ///     y: 2,
    ///     z: 3,
    /// };
    /// let cmd_data = unsafe {
    ///     std::slice::from_raw_parts(&cmd as *const _ as *const _, CMD_SIZE)
    /// };
    ///
    /// let args_buf_flags = vk::BufferUsageFlags::STORAGE_BUFFER;
    /// let args_buf = Buffer::create_from_slice(&device, args_buf_flags, cmd_data)?;
    /// let args_buf_node = my_graph.bind_node(args_buf);
    ///
    /// my_graph.begin_pass("fill my_buf_node with data")
    ///         .bind_pipeline(&my_compute_pipeline)
    ///         .read_node(args_buf_node)
    ///         .write_descriptor(0, my_buf_node)
    ///         .record_compute(move |compute, bindings| {
    ///             compute.dispatch_indirect(args_buf_node, 0);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [Dispatch]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/vkCmdDispatchIndirect.html
    /// [VkDispatchIndirectCommand]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkDispatchIndirectCommand.html
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

    /// Updates push constants.
    ///
    /// Push constants represent a high speed path to modify constant data in pipelines that is
    /// expected to outperform memory-backed resource updates.
    ///
    /// Push constant values can be updated incrementally, causing shader stages to read the new
    /// data for push constants modified by this command, while still reading the previous data for
    /// push constants not modified by this command.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 450
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint the_answer;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add bindings to read/write things!
    /// }
    /// # "#, comp);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let shader = [0u8; 1];
    /// # let pipeline_info = ComputePipelineInfo::new(shader.as_slice());
    /// # let my_compute_pipeline = Arc::new(ComputePipeline::create(&device, pipeline_info)?);
    /// # let mut my_graph = RenderGraph::new();
    /// my_graph.begin_pass("compute the ultimate question")
    ///         .bind_pipeline(&my_compute_pipeline)
    ///         .record_compute(move |compute, bindings| {
    ///             compute.push_constants(&[42])
    ///                    .dispatch(1, 1, 1);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
    pub fn push_constants(&self, data: &[u8]) -> &Self {
        self.push_constants_offset(0, data)
    }

    /// Updates push constants starting at the given `offset`.
    ///
    /// Behaves similary to [`Compute::push_constants`] except that `offset` describes the position
    /// at which `data` updates the push constants of the currently bound pipeline. This may be used
    /// to update a subset or single field of previously set push constant data.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 450
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint some_val1;
    ///     layout(offset = 4) uint some_val2;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add bindings to read/write things!
    /// }
    /// # "#, comp);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let shader = [0u8; 1];
    /// # let pipeline_info = ComputePipelineInfo::new(shader.as_slice());
    /// # let my_compute_pipeline = Arc::new(ComputePipeline::create(&device, pipeline_info)?);
    /// # let mut my_graph = RenderGraph::new();
    /// my_graph.begin_pass("calculate the wow factor")
    ///         .bind_pipeline(&my_compute_pipeline)
    ///         .record_compute(move |compute, bindings| {
    ///             compute.push_constants(&[0x00, 0x00])
    ///                    .dispatch(1, 1, 1)
    ///                    .push_constants_offset(4, &[0xff])
    ///                    .dispatch(1, 1, 1);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
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

/// Describes the SPIR-V binding index, and optionally a specific descriptor set
/// and array index.
///
/// Generally you might pass a function a descriptor using a simple integer:
///
/// ```rust
/// # fn my_func(_: usize, _: ()) {}
/// # let image = ();
/// let descriptor = 42;
/// my_func(descriptor, image);
/// ```
///
/// But also:
///
/// - `(0, 42)` for descriptor set `0` and binding index `42`
/// - `(42, [8])` for the same binding, but the 8th element
/// - `(0, 42, [8])` same as the previous example
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Descriptor {
    /// An array binding which includes an `offset` argument for the bound element.
    ArrayBinding(DescriptorSetIndex, BindingIndex, BindingOffset),

    /// A single binding.
    Binding(DescriptorSetIndex, BindingIndex),
}

impl Descriptor {
    pub(super) fn into_tuple(self) -> (DescriptorSetIndex, BindingIndex, BindingOffset) {
        match self {
            Self::ArrayBinding(descriptor_set_idx, binding_idx, binding_offset) => {
                (descriptor_set_idx, binding_idx, binding_offset)
            }
            Self::Binding(descriptor_set_idx, binding_idx) => (descriptor_set_idx, binding_idx, 0),
        }
    }

    pub(super) fn set(self) -> DescriptorSetIndex {
        let (res, _, _) = self.into_tuple();
        res
    }
}

impl From<BindingIndex> for Descriptor {
    fn from(val: BindingIndex) -> Self {
        Self::Binding(0, val)
    }
}

impl From<(DescriptorSetIndex, BindingIndex)> for Descriptor {
    fn from(tuple: (DescriptorSetIndex, BindingIndex)) -> Self {
        Self::Binding(tuple.0, tuple.1)
    }
}

impl From<(BindingIndex, [BindingOffset; 1])> for Descriptor {
    fn from(tuple: (BindingIndex, [BindingOffset; 1])) -> Self {
        Self::ArrayBinding(0, tuple.0, tuple.1[0])
    }
}

impl From<(DescriptorSetIndex, BindingIndex, [BindingOffset; 1])> for Descriptor {
    fn from(tuple: (DescriptorSetIndex, BindingIndex, [BindingOffset; 1])) -> Self {
        Self::ArrayBinding(tuple.0, tuple.1, tuple.2[0])
    }
}

/// Recording interface for drawing commands.
///
/// This structure provides a strongly-typed set of methods which allow rasterization shader code to
/// be executed. An instance of `Draw` is provided to the closure parameter of
/// [`PipelinePassRef::record_subpass`] which may be accessed by binding a [`GraphicPipeline`] to a
/// render pass.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{Device, DriverConfig, DriverError};
/// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
/// # use screen_13::driver::image::{Image, ImageInfo};
/// # use screen_13::graph::RenderGraph;
/// # use screen_13::driver::shader::Shader;
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let my_frag_code = [0u8; 1];
/// # let my_vert_code = [0u8; 1];
/// # let vert = Shader::new_vertex(my_vert_code.as_slice());
/// # let frag = Shader::new_fragment(my_frag_code.as_slice());
/// # let info = GraphicPipelineInfo::default();
/// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
/// # let mut my_graph = RenderGraph::new();
/// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
/// # let swapchain_image = my_graph.bind_node(Image::create(&device, info)?);
/// my_graph.begin_pass("my draw pass")
///         .bind_pipeline(&my_graphic_pipeline)
///         .store_color(0, swapchain_image)
///         .record_subpass(move |subpass, bindings| {
///             // During this closure we have access to the draw methods!
///         });
/// # Ok(()) }
/// ```
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
    /// Bind an index buffer to the current pass.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// # let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// # let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// # let info = GraphicPipelineInfo::default();
    /// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// # let swapchain_image = my_graph.bind_node(Image::create(&device, info)?);
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::INDEX_BUFFER);
    /// # let my_idx_buf = Buffer::create(&device, buf_info)?;
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::VERTEX_BUFFER);
    /// # let my_vtx_buf = Buffer::create(&device, buf_info)?;
    /// # let my_idx_buf = my_graph.bind_node(my_idx_buf);
    /// # let my_vtx_buf = my_graph.bind_node(my_vtx_buf);
    /// my_graph.begin_pass("my indexed geometry draw pass")
    ///         .bind_pipeline(&my_graphic_pipeline)
    ///         .store_color(0, swapchain_image)
    ///         .read_node(my_idx_buf)
    ///         .read_node(my_vtx_buf)
    ///         .record_subpass(move |subpass, bindings| {
    ///             subpass.bind_index_buffer(my_idx_buf, vk::IndexType::UINT16)
    ///                    .bind_vertex_buffer(my_vtx_buf)
    ///                    .draw_indexed(42, 1, 0, 0, 0);
    ///         });
    /// # Ok(()) }
    /// ```
    pub fn bind_index_buffer(
        &self,
        buffer: impl Into<AnyBufferNode>,
        index_ty: vk::IndexType,
    ) -> &Self {
        self.bind_index_buffer_offset(buffer, index_ty, 0)
    }

    /// Bind an index buffer to the current pass.
    ///
    /// Behaves similarly to `bind_index_buffer` except that `offset` is the starting offset in
    /// bytes within `buffer` used in index buffer address calculations.
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

    /// Bind a vertex buffer to the current pass.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::VERTEX_BUFFER);
    /// # let my_vtx_buf = Buffer::create(&device, buf_info)?;
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// # let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// # let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// # let info = GraphicPipelineInfo::default();
    /// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// # let swapchain_image = my_graph.bind_node(Image::create(&device, info)?);
    /// # let my_vtx_buf = my_graph.bind_node(my_vtx_buf);
    /// my_graph.begin_pass("my unindexed geometry draw pass")
    ///         .bind_pipeline(&my_graphic_pipeline)
    ///         .store_color(0, swapchain_image)
    ///         .read_node(my_vtx_buf)
    ///         .record_subpass(move |subpass, bindings| {
    ///             subpass.bind_vertex_buffer(my_vtx_buf)
    ///                    .draw(42, 1, 0, 0);
    ///         });
    /// # Ok(()) }
    /// ```
    pub fn bind_vertex_buffer(&self, buffer: impl Into<AnyBufferNode>) -> &Self {
        self.bind_vertex_buffer_offset(buffer, 0)
    }

    /// Bind a vertex buffer to the current pass.
    ///
    /// Behaves similarly to `bind_vertex_buffer` except the vertex input binding is updated to
    /// start at `offset` from the start of `buffer`.
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

    /// Binds multiple vertex buffers to the current pass, starting at the given `first_binding`.
    ///
    /// Each vertex input binding in `buffers` specifies an offset from the start of the
    /// corresponding buffer.
    ///
    /// The vertex input attributes that use each of these bindings will use these updated addresses
    /// in their address calculations for subsequent drawing commands.
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

    /// Draw unindexed primitives.
    ///
    /// When the command is executed, primitives are assembled using the current primitive topology
    /// and `vertex_count` consecutive vertex indices with the first `vertex_index` value equal to
    /// `first_vertex`. The primitives are drawn `instance_count` times with `instance_index`
    /// starting with `first_instance` and increasing sequentially for each instance.
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

    /// Draw indexed primitives.
    ///
    /// When the command is executed, primitives are assembled using the current primitive topology
    /// and `index_count` vertices whose indices are retrieved from the index buffer. The index
    /// buffer is treated as an array of tightly packed unsigned integers of size defined by the
    /// `index_ty` parameter with which the buffer was bound.
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

    /// Draw primitives with indirect parameters and indexed vertices.
    ///
    /// `draw_indexed_indirect` behaves similarly to `draw_indexed` except that the parameters are
    /// read by the device from `buffer` during execution. `draw_count` draws are executed by the
    /// command, with parameters taken from `buffer` starting at `offset` and increasing by `stride`
    /// bytes for each successive draw. The parameters of each draw are encoded in an array of
    /// [`vk::DrawIndexedIndirectCommand`] structures.
    ///
    /// If `draw_count` is less than or equal to one, `stride` is ignored.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use std::mem::size_of;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// # let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// # let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// # let info = GraphicPipelineInfo::default();
    /// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
    /// # let mut my_graph = RenderGraph::new();
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::INDEX_BUFFER);
    /// # let my_idx_buf = Buffer::create(&device, buf_info)?;
    /// # let buf_info = BufferInfo::new(8, vk::BufferUsageFlags::VERTEX_BUFFER);
    /// # let my_vtx_buf = Buffer::create(&device, buf_info)?;
    /// # let my_idx_buf = my_graph.bind_node(my_idx_buf);
    /// # let my_vtx_buf = my_graph.bind_node(my_vtx_buf);
    /// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// # let swapchain_image = my_graph.bind_node(Image::create(&device, info)?);
    /// const CMD_SIZE: usize = size_of::<vk::DrawIndexedIndirectCommand>();
    ///
    /// let cmd = vk::DrawIndexedIndirectCommand {
    ///     index_count: 3,
    ///     instance_count: 1,
    ///     first_index: 0,
    ///     vertex_offset: 0,
    ///     first_instance: 0,
    /// };
    /// let cmd_data = unsafe {
    ///     std::slice::from_raw_parts(&cmd as *const _ as *const _, CMD_SIZE)
    /// };
    ///
    /// let buf_flags = vk::BufferUsageFlags::STORAGE_BUFFER;
    /// let buf = Buffer::create_from_slice(&device, buf_flags, cmd_data)?;
    /// let buf_node = my_graph.bind_node(buf);
    ///
    /// my_graph.begin_pass("draw a single triangle")
    ///         .bind_pipeline(&my_graphic_pipeline)
    ///         .store_color(0, swapchain_image)
    ///         .read_node(my_idx_buf)
    ///         .read_node(my_vtx_buf)
    ///         .read_node(buf_node)
    ///         .record_subpass(move |subpass, bindings| {
    ///             subpass.bind_index_buffer(my_idx_buf, vk::IndexType::UINT16)
    ///                    .bind_vertex_buffer(my_vtx_buf)
    ///                    .draw_indexed_indirect(buf_node, 0, 1, 0);
    ///         });
    /// # Ok(()) }
    /// ```
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

    /// Draw primitives with indirect parameters, indexed vertices, and draw count.
    ///
    /// `draw_indexed_indirect_count` behaves similarly to `draw_indexed_indirect` except that the
    /// draw count is read by the device from `buffer` during execution. The command will read an
    /// unsigned 32-bit integer from `count_buf` located at `count_buf_offset` and use this as the
    /// draw count.
    ///
    /// `max_draw_count` specifies the maximum number of draws that will be executed. The actual
    /// number of executed draw calls is the minimum of the count specified in `count_buf` and
    /// `max_draw_count`.
    ///
    /// `stride` is the byte stride between successive sets of draw parameters.
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

    /// Draw primitives with indirect parameters and unindexed vertices.
    ///
    /// Behaves otherwise similar to [`Draw::draw_indexed_indirect`].
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

    /// Draw primitives with indirect parameters, unindexed vertices, and draw count.
    ///
    /// Behaves otherwise similar to [`Draw::draw_indexed_indirect_count`].
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

    /// Updates push constants.
    ///
    /// Push constants represent a high speed path to modify constant data in pipelines that is
    /// expected to outperform memory-backed resource updates.
    ///
    /// Push constant values can be updated incrementally, causing shader stages to read the new
    /// data for push constants modified by this command, while still reading the previous data for
    /// push constants not modified by this command.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 450
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint the_answer;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add code!
    /// }
    /// # "#, vert);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # use screen_13::driver::shader::Shader;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// # let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// # let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// # let info = GraphicPipelineInfo::default();
    /// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
    /// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// # let swapchain_image = Image::create(&device, info)?;
    /// # let mut my_graph = RenderGraph::new();
    /// # let swapchain_image = my_graph.bind_node(swapchain_image);
    /// my_graph.begin_pass("draw a quad")
    ///         .bind_pipeline(&my_graphic_pipeline)
    ///         .store_color(0, swapchain_image)
    ///         .record_subpass(move |subpass, bindings| {
    ///             subpass.push_constants(&[42])
    ///                    .draw(6, 1, 0, 0);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
    pub fn push_constants(&self, data: &[u8]) -> &Self {
        self.push_constants_offset(0, data)
    }

    /// Updates push constants starting at the given `offset`.
    ///
    /// Behaves similary to [`Draw::push_constants`] except that `offset` describes the position at
    /// which `data` updates the push constants of the currently bound pipeline. This may be used to
    /// update a subset or single field of previously set push constant data.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 450
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint some_val1;
    ///     layout(offset = 4) uint some_val2;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add code!
    /// }
    /// # "#, vert);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # use screen_13::graph::RenderGraph;
    /// # use screen_13::driver::shader::Shader;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// # let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// # let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// # let info = GraphicPipelineInfo::default();
    /// # let my_graphic_pipeline = Arc::new(GraphicPipeline::create(&device, info, [vert, frag])?);
    /// # let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// # let swapchain_image = Image::create(&device, info)?;
    /// # let mut my_graph = RenderGraph::new();
    /// # let swapchain_image = my_graph.bind_node(swapchain_image);
    /// my_graph.begin_pass("draw a quad")
    ///         .bind_pipeline(&my_graphic_pipeline)
    ///         .store_color(0, swapchain_image)
    ///         .record_subpass(move |subpass, bindings| {
    ///             subpass.push_constants(&[0x00, 0x00])
    ///                    .draw(6, 1, 0, 0)
    ///                    .push_constants_offset(4, &[0xff])
    ///                    .draw(6, 1, 0, 0);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
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

    /// Set scissor rectangle dynamically for a pass.
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

    /// Set scissor rectangles dynamically for a pass.
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

    /// Set the viewport dynamically for a pass.
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

    /// Set the viewports dynamically for a pass.
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

/// A general render pass which may contain acceleration structure commands, general commands, or
/// have pipeline bound to then record commands specific to those pipeline types.
pub struct PassRef<'a> {
    pub(super) exec_idx: usize,
    pub(super) graph: &'a mut RenderGraph,
    pub(super) pass_idx: usize,
}

impl<'a> PassRef<'a> {
    pub(super) fn new(graph: &'a mut RenderGraph, name: String) -> PassRef<'a> {
        let pass_idx = graph.passes.len();
        graph.passes.push(Pass {
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

    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PassRef::read_node`] or [`PassRef::write_node`].
    pub fn access_node(mut self, node: impl Node + Information, access: AccessType) -> Self {
        self.access_node_mut(node, access);

        self
    }

    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PassRef::read_node_mut`] or
    /// [`PassRef::write_node_mut`].
    pub fn access_node_mut(&mut self, node: impl Node + Information, access: AccessType) {
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
    }

    /// Informs the pass that the next recorded command buffer will read or write the `subresource`
    /// of `node` using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PassRef::read_node`] or [`PassRef::write_node`].
    pub fn access_node_subrange<N>(
        mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View,
    {
        self.access_node_subrange_mut(node, access, subresource);

        self
    }

    /// Informs the pass that the next recorded command buffer will read or write the `subresource`
    /// of `node` using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PassRef::read_node`] or [`PassRef::write_node`].
    pub fn access_node_subrange_mut<N>(
        &mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) where
        N: View,
    {
        self.push_node_access(node, access, Some(subresource.into().into()));
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

    /// Binds a Vulkan acceleration structure, buffer, or image to the graph associated with this
    /// pass.
    ///
    /// Bound nodes may be used in passes for pipeline and shader operations.
    pub fn bind_node<'b, B>(&'b mut self, binding: B) -> <B as Edge<RenderGraph>>::Result
    where
        B: Edge<RenderGraph>,
        B: Bind<&'b mut RenderGraph, <B as Edge<RenderGraph>>::Result>,
    {
        self.graph.bind_node(binding)
    }

    /// Binds a [`ComputePipeline`], [`GraphicPipeline`], or [`RayTracePipeline`] to the current
    /// pass, allowing for strongly typed access to the related functions.
    pub fn bind_pipeline<B>(self, binding: B) -> <B as Edge<Self>>::Result
    where
        B: Edge<Self>,
        B: Bind<Self, <B as Edge<Self>>::Result>,
    {
        binding.bind(self)
    }

    /// Returns information used to crate a node.
    pub fn node_info<N>(&self, node: N) -> <N as Information>::Info
    where
        N: Information,
    {
        node.get(self.graph)
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

    /// Informs the pass that the next recorded command buffer will read the given `node` using
    /// [`AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer`].
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PassRef::access_node`].
    pub fn read_node(mut self, node: impl Node + Information) -> Self {
        self.read_node_mut(node);

        self
    }

    /// Informs the pass that the next recorded command buffer will read the given `node` using
    /// [`AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer`].
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PassRef::access_node`].
    pub fn read_node_mut(&mut self, node: impl Node + Information) {
        self.access_node_mut(
            node,
            AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer,
        );
    }

    /// Begin recording an acceleration structure command buffer.
    ///
    /// This is the entry point for building and updating an [`AccelerationStructure`] instance.
    pub fn record_acceleration(
        mut self,
        func: impl FnOnce(Acceleration<'_>, Bindings<'_>) + Send + 'static,
    ) -> Self {
        self.push_execute(move |device, cmd_buf, bindings| {
            func(
                Acceleration {
                    bindings,
                    cmd_buf,
                    device,
                },
                bindings,
            );
        });

        self
    }

    /// Begin recording a general command buffer.
    ///
    /// The provided closure allows you to run any Vulkan code, or interoperate with other Vulkan
    /// code and interfaces.
    pub fn record_cmd_buf(
        mut self,
        func: impl FnOnce(&Device, vk::CommandBuffer, Bindings<'_>) + Send + 'static,
    ) -> Self {
        self.push_execute(func);

        self
    }

    /// Finalize the recording of this pass and return to the `RenderGraph` where you may record
    /// additional passes.
    pub fn submit_pass(self) -> &'a mut RenderGraph {
        // If nothing was done in this pass we can just ignore it
        if self.exec_idx == 0 {
            self.graph.passes.pop();
        }

        self.graph
    }

    /// Informs the pass that the next recorded command buffer will write the given `node` using
    /// [`AccessType::AnyShaderWrite`].
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PassRef::access_node`].
    pub fn write_node(mut self, node: impl Node + Information) -> Self {
        self.write_node_mut(node);

        self
    }

    /// Informs the pass that the next recorded command buffer will write the given `node` using
    /// [`AccessType::AnyShaderWrite`].
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PassRef::access_node`].
    pub fn write_node_mut(&mut self, node: impl Node + Information) {
        self.access_node_mut(node, AccessType::AnyShaderWrite);
    }
}

/// A render pass which has been bound to a particular compute, graphic, or ray-trace pipeline.
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
    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// at the specified shader descriptor using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_descriptor`] or
    /// [`PipelinePassRef::write_descriptor`].
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

    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// at the specified shader descriptor using `access`. The node will be interpreted using
    /// `view_info`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_descriptor_as`] or
    /// [`PipelinePassRef::write_descriptor_as`].
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

    /// Informs the pass that the next recorded command buffer will read or write the `subresource`
    /// of `node` at the specified shader descriptor using `access`. The node will be interpreted
    /// using `view_info`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_descriptor_subrange`] or
    /// [`PipelinePassRef::write_descriptor_subrange`].
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

    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_node`] or
    /// [`PipelinePassRef::write_node`].
    pub fn access_node(mut self, node: impl Node + Information, access: AccessType) -> Self {
        self.access_node_mut(node, access);

        self
    }

    /// Informs the pass that the next recorded command buffer will read or write the given `node`
    /// using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_node_mut`] or
    /// [`PipelinePassRef::write_node_mut`].
    pub fn access_node_mut(&mut self, node: impl Node + Information, access: AccessType) {
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
    }

    /// Informs the pass that the next recorded command buffer will read or write the `subresource`
    /// of `node` using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_node_subrange`] or
    /// [`PipelinePassRef::write_node_subrange`].
    pub fn access_node_subrange<N>(
        mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) -> Self
    where
        N: View,
    {
        self.access_node_subrange_mut(node, access, subresource);

        self
    }

    /// Informs the pass that the next recorded command buffer will read or write the `subresource`
    /// of `node` using `access`.
    ///
    /// This function must be called for `node` before it is read or written within a `record`
    /// function. For general purpose access, see [`PipelinePassRef::read_node_subrange_mut`] or
    /// [`PipelinePassRef::write_node_subrange_mut`].
    pub fn access_node_subrange_mut<N>(
        &mut self,
        node: N,
        access: AccessType,
        subresource: impl Into<N::Subresource>,
    ) where
        N: View,
    {
        self.pass
            .push_node_access(node, access, Some(subresource.into().into()));
    }

    /// Binds a Vulkan acceleration structure, buffer, or image to the graph associated with this
    /// pass.
    ///
    /// Bound nodes may be used in passes for pipeline and shader operations.
    pub fn bind_node<'b, B>(&'b mut self, binding: B) -> <B as Edge<RenderGraph>>::Result
    where
        B: Edge<RenderGraph>,
        B: Bind<&'b mut RenderGraph, <B as Edge<RenderGraph>>::Result>,
    {
        self.pass.graph.bind_node(binding)
    }

    /// Returns information used to crate a node.
    pub fn node_info<N>(&self, node: N) -> <N as Information>::Info
    where
        N: Information,
    {
        node.get(self.pass.graph)
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

    /// Informs the pass that the next recorded command buffer will read the given `node` at the
    /// specified shader descriptor.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor`].
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

    /// Informs the pass that the next recorded command buffer will read the given `node` at the
    /// specified shader descriptor. The node will be interpreted using `view_info`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor_as`].
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

    /// Informs the pass that the next recorded command buffer will read the `subresource` of `node`
    /// at the specified shader descriptor. The node will be interpreted using `view_info`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor_subrange`].
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

    /// Informs the pass that the next recorded command buffer will read the given `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node`].
    pub fn read_node(mut self, node: impl Node + Information) -> Self {
        self.read_node_mut(node);

        self
    }

    /// Informs the pass that the next recorded command buffer will read the given `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_mut`].
    pub fn read_node_mut(&mut self, node: impl Node + Information) {
        let access = <T as Access>::DEFAULT_READ;
        self.access_node_mut(node, access);
    }

    /// Informs the pass that the next recorded command buffer will read the `subresource` of
    /// `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_subrange`].
    pub fn read_node_subrange<N>(mut self, node: N, subresource: impl Into<N::Subresource>) -> Self
    where
        N: View,
    {
        self.read_node_subrange_mut(node, subresource);

        self
    }

    /// Informs the pass that the next recorded command buffer will read the `subresource` of
    /// `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is read within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_subrange_mut`].
    pub fn read_node_subrange_mut<N>(&mut self, node: N, subresource: impl Into<N::Subresource>)
    where
        N: View,
    {
        let access = <T as Access>::DEFAULT_READ;
        self.access_node_subrange_mut(node, access, subresource);
    }

    /// Finalizes a pass and returns the render graph so that additional passes may be added.
    pub fn submit_pass(self) -> &'a mut RenderGraph {
        self.pass.submit_pass()
    }

    /// Informs the pass that the next recorded command buffer will write the given `node` at the
    /// specified shader descriptor.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor`].
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

    /// Informs the pass that the next recorded command buffer will write the given `node` at the
    /// specified shader descriptor. The node will be interpreted using `view_info`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor_as`].
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

    /// Informs the pass that the next recorded command buffer will write the `subresource` of
    /// `node` at the specified shader descriptor. The node will be interpreted using `view_info`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_descriptor_subrange`].
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

    /// Informs the pass that the next recorded command buffer will write the given `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node`].
    pub fn write_node(mut self, node: impl Node + Information) -> Self {
        self.write_node_mut(node);

        self
    }

    /// Informs the pass that the next recorded command buffer will write the given `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_mut`].
    pub fn write_node_mut(&mut self, node: impl Node + Information) {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_node_mut(node, access);
    }

    /// Informs the pass that the next recorded command buffer will write the `subresource` of
    /// `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_subrange`].
    pub fn write_node_subrange<N>(mut self, node: N, subresource: impl Into<N::Subresource>) -> Self
    where
        N: View,
    {
        self.write_node_subrange_mut(node, subresource);

        self
    }

    /// Informs the pass that the next recorded command buffer will write the `subresource` of
    /// `node`.
    ///
    /// The [`AccessType`] is inferred by the currently bound pipeline. See [`Access`] for details.
    ///
    /// This function must be called for `node` before it is written within a `record` function. For
    /// more specific access, see [`PipelinePassRef::access_node_subrange_mut`].
    pub fn write_node_subrange_mut<N>(&mut self, node: N, subresource: impl Into<N::Subresource>)
    where
        N: View,
    {
        let access = <T as Access>::DEFAULT_WRITE;
        self.access_node_subrange_mut(node, access, subresource);
    }
}

impl<'a> PipelinePassRef<'a, ComputePipeline> {
    /// Begin recording a computing command buffer.
    pub fn record_compute(
        mut self,
        func: impl FnOnce(Compute<'_>, Bindings<'_>) + Send + 'static,
    ) -> Self {
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
            func(
                Compute {
                    bindings,
                    cmd_buf,
                    device,
                    pipeline,
                },
                bindings,
            );
        });

        self
    }
}

impl<'a> PipelinePassRef<'a, GraphicPipeline> {
    /// Specifies `VK_ATTACHMENT_LOAD_OP_DONT_CARE` for the render pass attachment, and loads an
    /// image into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call attach before resolve or store.
    pub fn attach_color(
        self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.attach_color_as(attachment_idx, image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_DONT_CARE` for the render pass attachment, and loads an
    /// image into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call attach before resolve or store.
    pub fn attach_color_as(
        mut self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .color_attachments
            .insert(
                attachment_idx,
                Attachment {
                    aspect_mask: image_view_info.aspect_mask,
                    format: image_view_info.fmt,
                    sample_count,
                    target: node_idx,
                },
            );

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_DONT_CARE` for the render pass attachment, and loads an
    /// image into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call attach before resolve or store.
    pub fn attach_depth_stencil(self, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.attach_depth_stencil_as(image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_DONT_CARE` for the render pass attachment, and loads an
    /// image into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call attach before resolve or store.
    pub fn attach_depth_stencil_as(
        mut self,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .depth_stencil_load = Some(Attachment {
            aspect_mask: image_view_info.aspect_mask,
            format: image_view_info.fmt,
            sample_count,
            target: node_idx,
        });

        self.pass.push_node_access(
            image,
            if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
            {
                AccessType::DepthStencilAttachmentWrite
            } else if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH)
            {
                AccessType::DepthAttachmentWriteStencilReadOnly
            } else {
                AccessType::StencilAttachmentWriteDepthReadOnly
            },
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_color(
        self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
    ) -> Self {
        self.clear_color_value(attachment_idx, image, [0.0, 0.0, 0.0, 0.0])
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_color_value(
        self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        color: impl Into<ClearColorValue>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.clear_color_value_as(attachment_idx, image, color, image_view_info)
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_color_value_as(
        mut self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        color: impl Into<ClearColorValue>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        let color = color.into();

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .color_clears
            .insert(
                attachment_idx,
                (
                    Attachment {
                        aspect_mask: image_view_info.aspect_mask,
                        format: image_view_info.fmt,
                        sample_count,
                        target: node_idx,
                    },
                    color,
                ),
            );

        self.pass.push_node_access(
            image,
            AccessType::ColorAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_depth_stencil(self, image: impl Into<AnyImageNode>) -> Self {
        self.clear_depth_stencil_value(image, 1.0, 0)
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_depth_stencil_value(
        self,
        image: impl Into<AnyImageNode>,
        depth: f32,
        stencil: u32,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.clear_depth_stencil_value_as(image, depth, stencil, image_view_info)
    }

    /// Clears the render pass attachment of any existing data.
    ///
    /// _NOTE:_ Order matters, call clear before resolve or store.
    pub fn clear_depth_stencil_value_as(
        mut self,
        image: impl Into<AnyImageNode>,
        depth: f32,
        stencil: u32,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .depth_stencil_clear = Some((
            Attachment {
                aspect_mask: image_view_info.aspect_mask,
                format: image_view_info.fmt,
                sample_count,
                target: node_idx,
            },
            vk::ClearDepthStencilValue { depth, stencil },
        ));

        self.pass.push_node_access(
            image,
            AccessType::DepthStencilAttachmentWrite,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    fn image_info(&self, node_idx: NodeIndex) -> (vk::Format, SampleCount) {
        let image_info = self.pass.graph.bindings[node_idx]
            .as_driver_image()
            .unwrap()
            .info;

        (image_info.fmt, image_info.sample_count)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
    pub fn load_color(
        self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.load_color_as(attachment_idx, image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
    pub fn load_color_as(
        mut self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .color_loads
            .insert(
                attachment_idx,
                Attachment {
                    aspect_mask: image_view_info.aspect_mask,
                    format: image_view_info.fmt,
                    sample_count,
                    target: node_idx,
                },
            );

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
    pub fn load_depth_stencil(self, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.load_depth_stencil_as(image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_LOAD_OP_LOAD` for the render pass attachment, and loads an image
    /// into the framebuffer.
    ///
    /// _NOTE:_ Order matters, call load before resolve or store.
    pub fn load_depth_stencil_as(
        mut self,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .depth_stencil_load = Some(Attachment {
            aspect_mask: image_view_info.aspect_mask,
            format: image_view_info.fmt,
            sample_count,
            target: node_idx,
        });

        self.pass.push_node_access(
            image,
            AccessType::DepthStencilAttachmentRead,
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Begin recording a graphics command buffer.
    pub fn record_subpass(
        mut self,
        func: impl FnOnce(Draw<'_>, Bindings<'_>) + Send + 'static,
    ) -> Self {
        let pipeline = {
            let exec = self.pass.as_ref().execs.last().unwrap();
            let pipeline = exec.pipeline.as_ref().unwrap().unwrap_graphic();

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
                    func(
                        Draw {
                            bindings,
                            buffers,
                            cmd_buf,
                            device,
                            offsets,
                            pipeline,
                            rects,
                            viewports,
                        },
                        bindings,
                    );
                },
            );
        });

        self
    }

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after clear or load.
    pub fn resolve_color(
        self,
        src_attachment_idx: AttachmentIndex,
        dst_attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.resolve_color_as(
            src_attachment_idx,
            dst_attachment_idx,
            image,
            image_view_info,
        )
    }

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after clear or load.
    pub fn resolve_color_as(
        mut self,
        src_attachment_idx: AttachmentIndex,
        dst_attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .color_resolves
            .insert(
                dst_attachment_idx,
                (
                    Attachment {
                        aspect_mask: image_view_info.aspect_mask,
                        format: image_view_info.fmt,
                        sample_count,
                        target: node_idx,
                    },
                    src_attachment_idx,
                ),
            );

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
    /// _NOTE:_ Order matters, call resolve after clear or load.
    pub fn resolve_depth_stencil(self, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.resolve_depth_stencil_as(image, image_view_info)
    }

    /// Resolves a multisample framebuffer to a non-multisample image for the render pass
    /// attachment.
    ///
    /// _NOTE:_ Order matters, call resolve after clear or load.
    pub fn resolve_depth_stencil_as(
        mut self,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .depth_stencil_resolve = Some(Attachment {
            aspect_mask: image_view_info.aspect_mask,
            format: image_view_info.fmt,
            sample_count,
            target: node_idx,
        });

        self.pass.push_node_access(
            image,
            if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
            {
                AccessType::DepthStencilAttachmentWrite
            } else if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH)
            {
                AccessType::DepthAttachmentWriteStencilReadOnly
            } else {
                AccessType::StencilAttachmentWriteDepthReadOnly
            },
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }

    /// Sets a particular depth/stencil mode.
    pub fn set_depth_stencil(mut self, depth_stencil: DepthStencilMode) -> Self {
        let pass = self.pass.as_mut();
        let exec = pass.execs.last_mut().unwrap();

        assert!(exec.depth_stencil.is_none());

        exec.depth_stencil = Some(depth_stencil);

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
    pub fn set_render_area(mut self, x: i32, y: i32, width: u32, height: u32) -> Self {
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
    /// _NOTE:_ Order matters, call store after clear or load.
    pub fn store_color(
        self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
    ) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.store_color_as(attachment_idx, image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after clear or load.
    pub fn store_color_as(
        mut self,
        attachment_idx: AttachmentIndex,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .color_stores
            .insert(
                attachment_idx,
                Attachment {
                    aspect_mask: image_view_info.aspect_mask,
                    format: image_view_info.fmt,
                    sample_count,
                    target: node_idx,
                },
            );

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
    /// _NOTE:_ Order matters, call store after clear or load.
    pub fn store_depth_stencil(self, image: impl Into<AnyImageNode>) -> Self {
        let image: AnyImageNode = image.into();
        let image_info = image.get(self.pass.graph);
        let image_view_info: ImageViewInfo = image_info.into();

        self.store_depth_stencil_as(image, image_view_info)
    }

    /// Specifies `VK_ATTACHMENT_STORE_OP_STORE` for the render pass attachment, and stores the
    /// rendered pixels into an image.
    ///
    /// _NOTE:_ Order matters, call store after clear or load.
    pub fn store_depth_stencil_as(
        mut self,
        image: impl Into<AnyImageNode>,
        image_view_info: impl Into<ImageViewInfo>,
    ) -> Self {
        let image = image.into();
        let image_view_info = image_view_info.into();
        let node_idx = image.index();
        let (_, sample_count) = self.image_info(node_idx);

        self.pass
            .as_mut()
            .execs
            .last_mut()
            .unwrap()
            .depth_stencil_store = Some(Attachment {
            aspect_mask: image_view_info.aspect_mask,
            format: image_view_info.fmt,
            sample_count,
            target: node_idx,
        });

        self.pass.push_node_access(
            image,
            if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
            {
                AccessType::DepthStencilAttachmentWrite
            } else if image_view_info
                .aspect_mask
                .contains(vk::ImageAspectFlags::DEPTH)
            {
                AccessType::DepthAttachmentWriteStencilReadOnly
            } else {
                AccessType::StencilAttachmentWriteDepthReadOnly
            },
            Some(Subresource::Image(image_view_info.into())),
        );

        self
    }
}

impl<'a> PipelinePassRef<'a, RayTracePipeline> {
    /// Begin recording a ray tracing command buffer.
    pub fn record_ray_trace(
        mut self,
        func: impl FnOnce(RayTrace<'_>, Bindings<'_>) + Send + 'static,
    ) -> Self {
        let exec = self.pass.as_ref().execs.last().unwrap();
        let pipeline = exec.pipeline.as_ref().unwrap().unwrap_ray_trace().clone();
        self.pass.push_execute(move |device, cmd_buf, bindings| {
            func(
                RayTrace {
                    cmd_buf,
                    device,
                    pipeline,
                },
                bindings,
            );
        });

        self
    }
}

/// Recording interface for ray tracing commands.
///
/// This structure provides a strongly-typed set of methods which allow ray trace shader code to be
/// executed. An instance of `RayTrace` is provided to the closure parameter of
/// [`PipelinePassRef::record_ray_trace`] which may be accessed by binding a [`RayTracePipeline`] to
/// a render pass.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{Device, DriverConfig, DriverError};
/// # use screen_13::driver::ray_trace::{RayTracePipeline, RayTracePipelineInfo, RayTraceShaderGroup};
/// # use screen_13::driver::shader::Shader;
/// # use screen_13::graph::RenderGraph;
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let info = RayTracePipelineInfo::new();
/// # let my_miss_code = [0u8; 1];
/// # let my_ray_trace_pipeline = Arc::new(RayTracePipeline::create(&device, info,
///     [Shader::new_miss(my_miss_code.as_slice())],
///     [RayTraceShaderGroup::new_general(0)],
/// )?);
/// # let mut my_graph = RenderGraph::new();
/// my_graph.begin_pass("my ray trace pass")
///         .bind_pipeline(&my_ray_trace_pipeline)
///         .record_ray_trace(move |ray_trace, bindings| {
///             // During this closure we have access to the ray trace methods!
///         });
/// # Ok(()) }
/// ```
pub struct RayTrace<'a> {
    cmd_buf: vk::CommandBuffer,
    device: &'a Device,
    pipeline: Arc<RayTracePipeline>,
}

impl<'a> RayTrace<'a> {
    /// Updates push constants.
    ///
    /// Push constants represent a high speed path to modify constant data in pipelines that is
    /// expected to outperform memory-backed resource updates.
    ///
    /// Push constant values can be updated incrementally, causing shader stages to read the new
    /// data for push constants modified by this command, while still reading the previous data for
    /// push constants not modified by this command.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 460
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint some_val;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add bindings to write things!
    /// }
    /// # "#, rchit, vulkan1_2);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::ray_trace::{RayTracePipeline, RayTracePipelineInfo, RayTraceShaderGroup};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let shader = [0u8; 1];
    /// # let info = RayTracePipelineInfo::new();
    /// # let my_miss_code = [0u8; 1];
    /// # let my_ray_trace_pipeline = Arc::new(RayTracePipeline::create(&device, info,
    /// #     [Shader::new_miss(my_miss_code.as_slice())],
    /// #     [RayTraceShaderGroup::new_general(0)],
    /// # )?);
    /// # let rgen_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let hit_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let miss_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let call_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let mut my_graph = RenderGraph::new();
    /// my_graph.begin_pass("draw a cornell box")
    ///         .bind_pipeline(&my_ray_trace_pipeline)
    ///         .record_ray_trace(move |ray_trace, bindings| {
    ///             ray_trace.push_constants(&[0xcb])
    ///                      .trace_rays(&rgen_sbt, &hit_sbt, &miss_sbt, &call_sbt, 320, 200, 1);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
    pub fn push_constants(&self, data: &[u8]) -> &Self {
        self.push_constants_offset(0, data)
    }

    /// Updates push constants starting at the given `offset`.
    ///
    /// Behaves similary to [`RayTrace::push_constants`] except that `offset` describes the position
    /// at which `data` updates the push constants of the currently bound pipeline. This may be used
    /// to update a subset or single field of previously set push constant data.
    ///
    /// # Device limitations
    ///
    /// See
    /// [`device.physical_device.props.limits.max_push_constants_size`](vk::PhysicalDeviceLimits)
    /// for the limits of the current device. You may also check [gpuinfo.org] for a listing of
    /// reported limits on other devices.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 460
    ///
    /// layout(push_constant) uniform PushConstants {
    ///     layout(offset = 0) uint some_val1;
    ///     layout(offset = 4) uint some_val2;
    /// } push_constants;
    ///
    /// void main()
    /// {
    ///     // TODO: Add bindings to write things!
    /// }
    /// # "#, rchit, vulkan1_2);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::ray_trace::{RayTracePipeline, RayTracePipelineInfo, RayTraceShaderGroup};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let shader = [0u8; 1];
    /// # let info = RayTracePipelineInfo::new();
    /// # let my_miss_code = [0u8; 1];
    /// # let my_ray_trace_pipeline = Arc::new(RayTracePipeline::create(&device, info,
    /// #     [Shader::new_miss(my_miss_code.as_slice())],
    /// #     [RayTraceShaderGroup::new_general(0)],
    /// # )?);
    /// # let rgen_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let hit_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let miss_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let call_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let mut my_graph = RenderGraph::new();
    /// my_graph.begin_pass("draw a cornell box")
    ///         .bind_pipeline(&my_ray_trace_pipeline)
    ///         .record_ray_trace(move |ray_trace, bindings| {
    ///             ray_trace.push_constants(&[0xcb, 0xff])
    ///                      .trace_rays(&rgen_sbt, &hit_sbt, &miss_sbt, &call_sbt, 320, 200, 1)
    ///                      .push_constants_offset(4, &[0xae])
    ///                      .trace_rays(&rgen_sbt, &hit_sbt, &miss_sbt, &call_sbt, 320, 200, 1);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [gpuinfo.org]: https://vulkan.gpuinfo.org/displaydevicelimit.php?name=maxPushConstantsSize&platform=all
    pub fn push_constants_offset(&self, offset: u32, data: &[u8]) -> &Self {
        for push_const in &self.pipeline.push_constants {
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
                trace!("data: {:#?}", data);

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

    // TODO: If the rayTraversalPrimitiveCulling or rayQuery features are enabled, the SkipTrianglesKHR and SkipAABBsKHR ray flags can be specified when tracing a ray. SkipTrianglesKHR and SkipAABBsKHR are mutually exclusive.

    /// Ray traces using the currently-bound [`RayTracePipeline`] and the given shader binding
    /// tables.
    ///
    /// Shader binding tables must be constructed according to this [example].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # use screen_13::driver::ray_trace::{RayTracePipeline, RayTracePipelineInfo, RayTraceShaderGroup};
    /// # use screen_13::driver::shader::Shader;
    /// # use screen_13::graph::RenderGraph;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let shader = [0u8; 1];
    /// # let info = RayTracePipelineInfo::new();
    /// # let my_miss_code = [0u8; 1];
    /// # let my_ray_trace_pipeline = Arc::new(RayTracePipeline::create(&device, info,
    /// #     [Shader::new_miss(my_miss_code.as_slice())],
    /// #     [RayTraceShaderGroup::new_general(0)],
    /// # )?);
    /// # let rgen_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let hit_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let miss_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let call_sbt = vk::StridedDeviceAddressRegionKHR { device_address: 0, stride: 0, size: 0 };
    /// # let mut my_graph = RenderGraph::new();
    /// my_graph.begin_pass("draw a cornell box")
    ///         .bind_pipeline(&my_ray_trace_pipeline)
    ///         .record_ray_trace(move |ray_trace, bindings| {
    ///             ray_trace.trace_rays(&rgen_sbt, &hit_sbt, &miss_sbt, &call_sbt, 320, 200, 1);
    ///         });
    /// # Ok(()) }
    /// ```
    ///
    /// [example]: https://github.com/attackgoat/screen-13/blob/master/examples/ray_trace.rs
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

    // pub fn trace_rays_indirect(
    //     &self,
    //     // _tlas: RayTraceAccelerationNode,
    //     _args_buf: BufferNode,
    //     _args_buf_offset: vk::DeviceSize,
    // ) -> &Self {
    //     // let mut pass = self.pass.as_mut();
    //     // let push_consts = take(&mut pass.push_consts);
    //     // let pipeline = Shared::clone(pass.pipelines.get(0).unwrap().unwrap_ray_trace());
    //     // let layout = pipeline.layout;

    //     // // TODO: Bind op to get a descriptor?

    //     // self.pass.push_execute(move |cmd_buf, bindings| unsafe {
    //     //     push_constants(push_consts, cmd_buf, layout);

    //     //     let args_buf_address = Buffer::device_address(&bindings[args_buf]) + args_buf_offset;
    //     //     cmd_buf
    //     //         .device
    //     //         .ray_trace_pipeline_ext
    //     //         .cmd_trace_rays_indirect(
    //     //             **cmd_buf,
    //     //             from_ref(&pipeline.shader_bindings.raygen),
    //     //             from_ref(&pipeline.shader_bindings.miss),
    //     //             from_ref(&pipeline.shader_bindings.hit),
    //     //             from_ref(&pipeline.shader_bindings.callable),
    //     //             args_buf_address,
    //     //         );
    //     // });
    //     self
    // }
}

/// Describes a portion of a resource which is bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subresource {
    /// Acceleration structures are bound whole.
    AccelerationStructure,

    /// Images may be partially bound.
    Image(ImageSubresource),

    /// Buffers may be partially bound.
    Buffer(BufferSubresource),
}

impl Subresource {
    pub(super) fn unwrap_buffer(self) -> BufferSubresource {
        if let Self::Buffer(subresource) = self {
            subresource
        } else {
            unreachable!();
        }
    }

    pub(super) fn unwrap_image(self) -> ImageSubresource {
        if let Self::Image(subresource) = self {
            subresource
        } else {
            unreachable!();
        }
    }
}

impl From<()> for Subresource {
    fn from(_: ()) -> Self {
        Self::AccelerationStructure
    }
}

impl From<ImageSubresource> for Subresource {
    fn from(subresource: ImageSubresource) -> Self {
        Self::Image(subresource)
    }
}

impl From<BufferSubresource> for Subresource {
    fn from(subresource: BufferSubresource) -> Self {
        Self::Buffer(subresource)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SubresourceAccess {
    pub access: AccessType,
    pub subresource: Option<Subresource>,
}

/// Allows for a resource to be reinterpreted as differently formatted data.
pub trait View: Node
where
    Self::Information: Clone,
    Self::Subresource: Into<Subresource>,
{
    /// The information about the resource interpretation.
    type Information;

    /// The portion of the resource which is bound.
    type Subresource;
}

impl View for AccelerationStructureNode {
    type Information = ();
    type Subresource = ();
}

impl View for AccelerationStructureLeaseNode {
    type Information = ();
    type Subresource = ();
}

impl View for AnyAccelerationStructureNode {
    type Information = ();
    type Subresource = ();
}

impl View for AnyBufferNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for AnyImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for BufferLeaseNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for BufferNode {
    type Information = BufferSubresource;
    type Subresource = BufferSubresource;
}

impl View for ImageLeaseNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for ImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

impl View for SwapchainImageNode {
    type Information = ImageViewInfo;
    type Subresource = ImageSubresource;
}

/// Describes the interpretation of a resource.
#[derive(Debug)]
pub enum ViewType {
    /// Acceleration structures are not reinterpreted.
    AccelerationStructure,

    /// Images may be interpreted as differently formatted images.
    Image(ImageViewInfo),

    /// Buffers may be interpreted as subregions of the same buffer.
    Buffer(Range<vk::DeviceSize>),
}

impl ViewType {
    pub(super) fn as_buffer(&self) -> Option<&Range<vk::DeviceSize>> {
        match self {
            Self::Buffer(view_info) => Some(view_info),
            _ => None,
        }
    }

    pub(super) fn as_image(&self) -> Option<&ImageViewInfo> {
        match self {
            Self::Image(view_info) => Some(view_info),
            _ => None,
        }
    }
}

impl From<()> for ViewType {
    fn from(_: ()) -> Self {
        Self::AccelerationStructure
    }
}

impl From<BufferSubresource> for ViewType {
    fn from(subresource: BufferSubresource) -> Self {
        Self::Buffer(subresource.start..subresource.end)
    }
}

impl From<ImageViewInfo> for ViewType {
    fn from(info: ImageViewInfo) -> Self {
        Self::Image(info)
    }
}

impl From<Range<vk::DeviceSize>> for ViewType {
    fn from(range: Range<vk::DeviceSize>) -> Self {
        Self::Buffer(range)
    }
}
