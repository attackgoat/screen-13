use {
    super::{
        Buffer, BufferInfo, DescriptorBindingMap, DescriptorSetLayout, Device, DriverError,
        PipelineDescriptorInfo, Shader,
    },
    crate::{as_u32_slice, ptr::Shared},
    archery::SharedPointerKind,
    ash::vk,
    glam::{Mat3, Vec3},
    log::{info, trace},
    parking_lot::Mutex,
    std::{collections::BTreeMap, ffi::CString, ops::Deref, thread::panicking},
};

#[derive(Debug)]
pub struct RayTraceAcceleration<P>
where
    P: SharedPointerKind,
{
    device: Shared<Device<P>, P>,
    accel_struct: vk::AccelerationStructureKHR,
    _buf: Buffer<P>,
}

impl<P> Deref for RayTraceAcceleration<P>
where
    P: SharedPointerKind,
{
    type Target = vk::AccelerationStructureKHR;

    fn deref(&self) -> &Self::Target {
        &self.accel_struct
    }
}

impl<P> Drop for RayTraceAcceleration<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device
                .accel_struct_ext
                .destroy_acceleration_structure(self.accel_struct, None);
        }
    }
}

#[derive(Clone, Debug)]
pub struct RayTraceAccelerationScratchBuffer<P>
where
    P: SharedPointerKind,
{
    device: Shared<Device<P>, P>,
    buf: Shared<Mutex<Buffer<P>>, P>,
}

impl<P> RayTraceAccelerationScratchBuffer<P>
where
    P: SharedPointerKind,
{
    pub const RT_SCRATCH_BUFFER_SIZE: u64 = 1024 * 1024 * 1440;

    pub fn create(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        trace!("create");

        let device = Shared::clone(device);
        let buf = Shared::new(Mutex::new(Buffer::create(
            &device,
            BufferInfo::new(
                Self::RT_SCRATCH_BUFFER_SIZE,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            )
            .build()
            .map_err(|_| DriverError::Unsupported)?,
        )?));

        Ok(Self { device, buf })
    }

    pub fn create_tlas(
        &self,
        _info: RayTraceTopAccelerationInfo<P>,
    ) -> Result<RayTraceAcceleration<P>, DriverError> {
        // let instances: Vec<RayTraceInstance> = desc
        //     .instances
        //     .iter()
        //     .map(|desc| {
        //         let blas_address = unsafe {
        //             self.device
        //                 .accel_struct_ext
        //                 .get_acceleration_structure_device_address(
        //                     &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
        //                         .acceleration_structure(desc.blas.accel_struct)
        //                         .build(),
        //                 )
        //         };
        //         let transform: [f32; 12] = [
        //             desc.rotation.x_axis.x,
        //             desc.rotation.y_axis.x,
        //             desc.rotation.z_axis.x,
        //             desc.position.x,
        //             desc.rotation.x_axis.y,
        //             desc.rotation.y_axis.y,
        //             desc.rotation.z_axis.y,
        //             desc.position.y,
        //             desc.rotation.x_axis.z,
        //             desc.rotation.y_axis.z,
        //             desc.rotation.z_axis.z,
        //             desc.position.z,
        //         ];

        //         RayTraceInstance::new(
        //             transform,
        //             desc.mesh_idx,
        //             u8::MAX,
        //             0,
        //             vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
        //             blas_address,
        //         )
        //     })
        //     .collect();
        // let instance_buf_len = size_of::<RayTraceInstance>() * instances.len().max(1);
        // let instance_buf = Buffer::create_with_data(
        //     &self.device,
        //     BufferDesc::new(
        //         instance_buf_len as u64,
        //         vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
        //             | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        //     )
        //     .build()
        //     .unwrap(),
        //     unsafe {
        //         (!instances.is_empty()).then(|| {
        //             std::slice::from_raw_parts(instances.as_ptr() as *const u8, instance_buf_len)
        //         })
        //     },
        // )?;
        // let device_address = Buffer::device_address(&instance_buf);
        // let geometry = vk::AccelerationStructureGeometryKHR::builder()
        //     .geometry_type(vk::GeometryTypeKHR::INSTANCES)
        //     .geometry(vk::AccelerationStructureGeometryDataKHR {
        //         instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
        //             .data(vk::DeviceOrHostAddressConstKHR { device_address })
        //             .build(),
        //     })
        //     .build();
        // let build_range_info = vk::AccelerationStructureBuildRangeInfoKHR::builder()
        //     .primitive_count(instances.len() as _)
        //     .build();
        // let geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
        //     .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
        //     .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
        //     .geometries(from_ref(&geometry))
        //     .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
        //     .build();
        // let max_primitive_count = instances.len() as _;

        // self.create_acceleration_structure(
        //     vk::AccelerationStructureTypeKHR::TOP_LEVEL,
        //     geometry_info,
        //     &[build_range_info],
        //     &[max_primitive_count],
        //     desc.preallocate_bytes,
        // )
        todo!();
    }

    pub fn create_blas(
        &self,
        info: &RayTraceBottomAccelerationDesc,
        _scratch_buf: &RayTraceAccelerationScratchBuffer<P>,
    ) -> Result<RayTraceAcceleration<P>, DriverError> {
        let geometries: Vec<vk::AccelerationStructureGeometryKHR> = info
            .geometries
            .iter()
            .map(|desc| -> vk::AccelerationStructureGeometryKHR {
                let part: RayTraceGeometryPart = desc.parts[0];
                let geometry = vk::AccelerationStructureGeometryKHR::builder()
                    .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                    .geometry(vk::AccelerationStructureGeometryDataKHR {
                        triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::builder()
                            .vertex_data(vk::DeviceOrHostAddressConstKHR {
                                device_address: desc.vertex_buf,
                            })
                            .vertex_stride(desc.vertex_stride as _)
                            .max_vertex(part.max_vertex)
                            .vertex_format(desc.vertex_fmt)
                            .index_data(vk::DeviceOrHostAddressConstKHR {
                                device_address: desc.idx_buf,
                            })
                            .index_type(vk::IndexType::UINT32) // TODO: Make parameter?
                            .build(),
                    })
                    .flags(vk::GeometryFlagsKHR::OPAQUE)
                    .build();

                geometry
            })
            .collect();
        let build_range_infos: Vec<vk::AccelerationStructureBuildRangeInfoKHR> = info
            .geometries
            .iter()
            .map(|desc| {
                vk::AccelerationStructureBuildRangeInfoKHR::builder()
                    .primitive_count(desc.parts[0].idx_count as u32 / 3)
                    .build()
            })
            .collect();
        let geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(geometries.as_slice())
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .build();
        let max_primitive_counts: Box<[_]> = info
            .geometries
            .iter()
            .map(|desc| desc.parts[0].idx_count as u32 / 3)
            .collect();

        self.create_acceleration_structure(
            vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            geometry_info,
            &build_range_infos,
            &max_primitive_counts,
            0,
        )
    }

    fn create_acceleration_structure(
        &self,
        ty: vk::AccelerationStructureTypeKHR,
        mut geometry_info: vk::AccelerationStructureBuildGeometryInfoKHR,
        _build_range_infos: &[vk::AccelerationStructureBuildRangeInfoKHR],
        max_primitive_counts: &[u32],
        preallocate_bytes: u64,
    ) -> Result<RayTraceAcceleration<P>, DriverError> {
        let mem_requirements = unsafe {
            self.device
                .accel_struct_ext
                .get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_info,
                    max_primitive_counts,
                )
        };

        info!(
            "Acceleration structure size: {}, scratch size: {}",
            mem_requirements.acceleration_structure_size, mem_requirements.build_scratch_size
        );

        let buf_len = preallocate_bytes.max(mem_requirements.acceleration_structure_size);
        let buf = Buffer::create(
            &self.device,
            BufferInfo::new(
                buf_len,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            )
            .build()
            .unwrap(),
        )?;
        let accel_info = vk::AccelerationStructureCreateInfoKHR::builder()
            .ty(ty)
            .buffer(*buf)
            .size(buf_len as u64)
            .build();

        unsafe {
            let accel_struct = self
                .device
                .accel_struct_ext
                .create_acceleration_structure(&accel_info, None)
                .map_err(|_| DriverError::Unsupported)?;
            let scratch_buf = self.buf.lock();

            // See `RT_SCRATCH_BUFFER_SIZE`
            assert!(
                mem_requirements.build_scratch_size <= scratch_buf.info.size,
                "todo: resize scratch"
            );

            geometry_info.dst_acceleration_structure = accel_struct;
            geometry_info.scratch_data = vk::DeviceOrHostAddressKHR {
                device_address: Buffer::device_address(&scratch_buf),
            };

            // self.with_setup_cb(|cb| {
            //     self.acceleration_structure_ext
            //         .cmd_build_acceleration_structures(
            //             cb,
            //             from_ref(&geometry_info),
            //             from_ref(&build_range_infos),
            //         );

            //     self.raw.cmd_pipeline_barrier(
            //         cb,
            //         vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            //         vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
            //         vk::DependencyFlags::empty(),
            //         &[vk::MemoryBarrier::builder()
            //             .src_access_mask(
            //                 vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
            //                     | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
            //             )
            //             .dst_access_mask(
            //                 vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
            //                     | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
            //             )
            //             .build()],
            //         &[],
            //         &[],
            //     );
            // });

            let device = Shared::clone(&self.device);

            Ok(RayTraceAcceleration {
                device,
                accel_struct,
                _buf: buf,
            })
        }
    }

    pub fn map_instances<'a>(
        &'a self,
        instances: impl Iterator<Item = RayTraceInstanceInfo<P>> + 'a,
    ) -> impl Iterator<Item = RayTraceInstance> + 'a {
        instances.map(|desc| {
            let blas_address = unsafe {
                self.device
                    .accel_struct_ext
                    .get_acceleration_structure_device_address(
                        &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
                            .acceleration_structure(desc.blas.accel_struct)
                            .build(),
                    )
            };
            let transform: [f32; 12] = [
                desc.rotation.x_axis.x,
                desc.rotation.y_axis.x,
                desc.rotation.z_axis.x,
                desc.position.x,
                desc.rotation.x_axis.y,
                desc.rotation.y_axis.y,
                desc.rotation.z_axis.y,
                desc.position.y,
                desc.rotation.x_axis.z,
                desc.rotation.y_axis.z,
                desc.rotation.z_axis.z,
                desc.position.z,
            ];

            RayTraceInstance::new(
                transform,
                desc.mesh_idx,
                u8::MAX,
                0,
                vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
                blas_address,
            )
        })
    }

    pub fn rebuild_tlas(
        &self,
        cb: vk::CommandBuffer,
        inst_buf_address: vk::DeviceAddress,
        instance_count: usize,
        tlas: &RayTraceAcceleration<P>,
    ) {
        use std::slice::from_ref;

        let geometry = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
                    .data(vk::DeviceOrHostAddressConstKHR {
                        device_address: inst_buf_address,
                    })
                    .build(),
            })
            .build();
        let build_range_infos = vec![vk::AccelerationStructureBuildRangeInfoKHR::builder()
            .primitive_count(instance_count as _)
            .build()];
        let mut geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(from_ref(&geometry))
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .build();

        let mem_requirements = unsafe {
            self.device
                .accel_struct_ext
                .get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_info,
                    from_ref(&(instance_count as u32)),
                )
        };
        let scratch_buf = self.buf.lock();

        assert!(
            mem_requirements.acceleration_structure_size <= scratch_buf.info.size,
            "todo: backing"
        );
        assert!(
            mem_requirements.build_scratch_size <= scratch_buf.info.size,
            "todo: scratch"
        );

        unsafe {
            geometry_info.dst_acceleration_structure = tlas.accel_struct;
            geometry_info.scratch_data = vk::DeviceOrHostAddressKHR {
                device_address: Buffer::device_address(&scratch_buf),
            };

            self.device
                .accel_struct_ext
                .cmd_build_acceleration_structures(
                    cb,
                    from_ref(&geometry_info),
                    from_ref(&build_range_infos.as_slice()),
                );
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::builder()
                    .src_access_mask(
                        vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
                            | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
                            | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                    )
                    .build()],
                &[],
                &[],
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct RayTraceBottomAccelerationDesc {
    pub geometries: Vec<RayTraceGeometryDesc>,
}

#[derive(Clone, Debug)]
pub struct RayTraceGeometryDesc {
    pub geometry_type: RayTraceGeometryType,
    pub vertex_buf: vk::DeviceAddress,
    pub idx_buf: vk::DeviceAddress,
    pub vertex_fmt: vk::Format,
    pub vertex_stride: usize,
    pub parts: Vec<RayTraceGeometryPart>,
}

#[derive(Clone, Copy, Debug)]
pub struct RayTraceGeometryPart {
    pub idx_count: usize,
    pub idx_offset: usize, // offset into the index buffer in bytes
    pub max_vertex: u32, // the highest index of a vertex that will be addressed by a build command using this structure
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum RayTraceGeometryType {
    Triangle = 0,
    BoundingBox = 1,
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct RayTraceInstance {
    transform: [f32; 12],
    instance_id_and_mask: u32,
    instance_shader_table_offset_and_flags: u32,
    blas_address: vk::DeviceAddress,
}

impl RayTraceInstance {
    fn new(
        transform: [f32; 12],
        id: u32,
        mask: u8,
        shader_table_offset: u32,
        flags: vk::GeometryInstanceFlagsKHR,
        blas_address: vk::DeviceAddress,
    ) -> Self {
        let mut res = Self {
            transform,
            instance_id_and_mask: 0,
            instance_shader_table_offset_and_flags: 0,
            blas_address,
        };
        res.set_id(id);
        res.set_mask(mask);
        res.set_shader_table_offset(shader_table_offset);
        res.set_flags(flags);

        res
    }

    fn set_id(&mut self, id: u32) {
        let id = id & 0x00ffffff;
        self.instance_id_and_mask |= id;
    }

    fn set_mask(&mut self, mask: u8) {
        let mask = mask as u32;
        self.instance_id_and_mask |= mask << 24;
    }

    fn set_shader_table_offset(&mut self, offset: u32) {
        let offset = offset & 0x00ffffff;
        self.instance_shader_table_offset_and_flags |= offset;
    }

    fn set_flags(&mut self, flags: vk::GeometryInstanceFlagsKHR) {
        let flags = flags.as_raw() as u32;
        self.instance_shader_table_offset_and_flags |= flags << 24;
    }
}

#[derive(Clone, Debug)]
pub struct RayTraceInstanceInfo<P>
where
    P: SharedPointerKind,
{
    pub blas: Shared<RayTraceAcceleration<P>, P>,
    pub mesh_idx: u32,
    pub position: Vec3,
    pub rotation: Mat3,
}

#[derive(Debug)]
pub struct RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo<P>,
    device: Shared<Device<P>, P>,
    pub layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    pub shader_bindings: RayTraceShaderBindings<P>,
}

impl<P> RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    pub fn create<S>(
        device: &Shared<Device<P>, P>,
        info: impl Into<RayTracePipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Result<Self, DriverError>
    where
        S: Into<Shader>,
    {
        let device = Shared::clone(device);
        let info = info.into();
        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let descriptor_bindings = Shader::merge_descriptor_bindings(
            shaders
                .iter()
                .map(|shader| shader.descriptor_bindings(&device))
                .collect::<Result<Vec<_>, _>>()?,
        );


        let stages = shaders
            .iter()
            .map(|shader| shader.stage)
            .reduce(|j, k| j | k)
            .unwrap_or_default();
        let descriptor_info =
            PipelineDescriptorInfo::create(&device, &descriptor_bindings, stages)?;
        let descriptor_set_layout_handles = descriptor_info
            .layouts
            .iter()
            .map(|(_, descriptor_set_layout)| **descriptor_set_layout)
            .collect::<Box<[_]>>();

        unsafe {
            let layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .set_layouts(&descriptor_set_layout_handles),
                    None,
                )
                .map_err(|_| DriverError::Unsupported)?;
            let mut entry_points: Vec<CString> = Vec::new(); // Keep entry point names alive, since build() forgets references.
            let mut prev_stage: Option<vk::ShaderStageFlags> = None;
            let mut shader_groups: Vec<vk::RayTracingShaderGroupCreateInfoKHR> = vec![];
            let mut shader_stages: Vec<vk::PipelineShaderStageCreateInfo> = vec![];
            let mut raygen_entry_count = 0;
            let mut miss_entry_count = 0;
            let mut hit_entry_count = 0;
            let create_shader_module =
                |info: &Shader| -> Result<(vk::ShaderModule, String), DriverError> {
                    let shader_module_create_info = vk::ShaderModuleCreateInfo {
                        code_size: info.spirv.len(),
                        p_code: info.spirv.as_ptr() as *const u32,
                        ..Default::default()
                    };
                    let shader_module = device
                        .create_shader_module(&shader_module_create_info, None)
                        .map_err(|_| DriverError::Unsupported)?;

                    Ok((shader_module, info.entry_name.clone()))
                };

            for desc in &shaders {
                let group_idx = shader_stages.len();

                match desc.stage {
                    vk::ShaderStageFlags::RAYGEN_KHR => {
                        assert!(
                            prev_stage == None
                                || prev_stage == Some(vk::ShaderStageFlags::RAYGEN_KHR)
                        );

                        raygen_entry_count += 1;

                        let (module, entry_point) = create_shader_module(desc)?;
                        entry_points.push(CString::new(entry_point).unwrap());

                        let entry_point = &**entry_points.last().unwrap();
                        let stage = vk::PipelineShaderStageCreateInfo::builder()
                            .stage(vk::ShaderStageFlags::RAYGEN_KHR)
                            .module(module)
                            .name(entry_point)
                            .build();
                        let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                            .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                            .general_shader(group_idx as _)
                            .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                            .any_hit_shader(vk::SHADER_UNUSED_KHR)
                            .intersection_shader(vk::SHADER_UNUSED_KHR)
                            .build();
                        shader_stages.push(stage);
                        shader_groups.push(group);
                    }
                    vk::ShaderStageFlags::MISS_KHR => {
                        assert!(
                            prev_stage == Some(vk::ShaderStageFlags::RAYGEN_KHR)
                                || prev_stage == Some(vk::ShaderStageFlags::MISS_KHR)
                        );

                        miss_entry_count += 1;

                        let (module, entry_point) = create_shader_module(desc)?;
                        entry_points.push(CString::new(entry_point).unwrap());

                        let entry_point = &**entry_points.last().unwrap();
                        let stage = vk::PipelineShaderStageCreateInfo::builder()
                            .stage(vk::ShaderStageFlags::MISS_KHR)
                            .module(module)
                            .name(entry_point)
                            .build();
                        let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                            .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                            .general_shader(group_idx as _)
                            .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                            .any_hit_shader(vk::SHADER_UNUSED_KHR)
                            .intersection_shader(vk::SHADER_UNUSED_KHR)
                            .build();
                        shader_stages.push(stage);
                        shader_groups.push(group);
                    }
                    vk::ShaderStageFlags::CLOSEST_HIT_KHR => {
                        assert!(
                            prev_stage == Some(vk::ShaderStageFlags::MISS_KHR)
                                || prev_stage == Some(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                        );

                        hit_entry_count += 1;

                        let (module, entry_point) = create_shader_module(desc)?;
                        entry_points.push(CString::new(entry_point).unwrap());

                        let entry_point = &**entry_points.last().unwrap();
                        let stage = vk::PipelineShaderStageCreateInfo::builder()
                            .stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                            .module(module)
                            .name(entry_point)
                            .build();
                        let group = vk::RayTracingShaderGroupCreateInfoKHR::builder()
                            .ty(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP)
                            .general_shader(vk::SHADER_UNUSED_KHR)
                            .closest_hit_shader(group_idx as _)
                            .any_hit_shader(vk::SHADER_UNUSED_KHR)
                            .intersection_shader(vk::SHADER_UNUSED_KHR)
                            .build();
                        shader_stages.push(stage);
                        shader_groups.push(group);
                    }
                    _ => unimplemented!(),
                }

                prev_stage = Some(desc.stage);
            }

            assert!(raygen_entry_count > 0);
            assert!(miss_entry_count > 0);

            let pipeline = device
                .ray_trace_pipeline_ext
                .create_ray_tracing_pipelines(
                    vk::DeferredOperationKHR::null(),
                    vk::PipelineCache::null(),
                    &[vk::RayTracingPipelineCreateInfoKHR::builder()
                        .stages(&shader_stages)
                        .groups(&shader_groups)
                        .max_pipeline_ray_recursion_depth(info.max_pipeline_ray_recursion_depth) // TODO
                        .layout(layout)
                        .build()],
                    None,
                )
                .map_err(|_| DriverError::Unsupported)?[0];
            let shader_bindings = RayTraceShaderBindings::create(
                &device,
                &RayTraceShaderBindingsDesc {
                    raygen_count: raygen_entry_count,
                    hit_count: hit_entry_count,
                    miss_count: miss_entry_count,
                },
                pipeline,
            )?;

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                layout,
                pipeline,
                shader_bindings,
            })
        }
    }
}

impl<P> Deref for RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    type Target = vk::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl<P> Drop for RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            // TODO: Drop other resources
            self.device.destroy_pipeline(self.pipeline, None);
        }
    }
}

#[derive(Clone, Debug)]
pub struct RayTracePipelineInfo {
    pub max_pipeline_ray_recursion_depth: u32,
}

impl Default for RayTracePipelineInfo {
    fn default() -> Self {
        Self {
            max_pipeline_ray_recursion_depth: 1,
        }
    }
}

impl RayTracePipelineInfo {
    pub fn max_pipeline_ray_recursion_depth(
        mut self,
        max_pipeline_ray_recursion_depth: u32,
    ) -> Self {
        self.max_pipeline_ray_recursion_depth = max_pipeline_ray_recursion_depth;
        self
    }
}

// TODO: Give this a nice impl so it's not constructed "bare"
#[derive(Clone, Debug)]
pub struct RayTraceTopAccelerationInfo<P>
where
    P: SharedPointerKind,
{
    pub instances: Vec<RayTraceInstanceInfo<P>>,
    pub preallocate_bytes: u64,
}

#[derive(Debug)]
pub struct RayTraceShaderBindings<P>
where
    P: SharedPointerKind,
{
    pub raygen_buf: Option<Buffer<P>>,
    pub raygen: vk::StridedDeviceAddressRegionKHR,
    pub miss_buf: Option<Buffer<P>>,
    pub miss: vk::StridedDeviceAddressRegionKHR,
    pub hit_buf: Option<Buffer<P>>,
    pub hit: vk::StridedDeviceAddressRegionKHR,
    pub callable_buf: Option<Buffer<P>>,
    pub callable: vk::StridedDeviceAddressRegionKHR,
}

impl<P> RayTraceShaderBindings<P>
where
    P: SharedPointerKind,
{
    fn create(
        _device: &Shared<Device<P>, P>,
        _info: &RayTraceShaderBindingsDesc,
        _pipeline: vk::Pipeline,
    ) -> Result<RayTraceShaderBindings<P>, DriverError> {
        // trace!("Creating ray tracing shader table: {:?}", desc);

        // let device = Shared::clone(device);
        // let shader_group_handle_size = device
        //     .ray_trace_pipeline_properties
        //     .shader_group_handle_size as usize;
        // let group_count = desc.raygen_count + desc.miss_count + desc.hit_count;
        // let group_handles_size = shader_group_handle_size * group_count as usize;
        // let group_handles: Vec<u8> = unsafe {
        //     device
        //         .ray_trace_pipeline_ext
        //         .get_ray_tracing_shader_group_handles(pipeline, 0, group_count, group_handles_size)
        //         .map_err(|_| DriverError::Unsupported)?
        // };
        // let prog_size = shader_group_handle_size;
        // let create_binding_table =
        //     |entry_offset: u32, entry_count: u32| -> Result<Option<Buffer<P>>, Error> {
        //         if entry_count == 0 {
        //             return Ok(None);
        //         }

        //         let mut sbt_data = vec![0u8; entry_count as usize * prog_size];

        //         for dst in 0..entry_count as usize {
        //             let src = dst + entry_offset as usize;
        //             sbt_data[dst * prog_size..dst * prog_size + shader_group_handle_size]
        //                 .copy_from_slice(
        //                     &group_handles[src * shader_group_handle_size
        //                         ..src * shader_group_handle_size + shader_group_handle_size],
        //                 );
        //         }

        //         Ok(Some(Buffer::create_with_data(
        //             &device,
        //             BufferDesc::new(
        //                 sbt_data.len() as u64,
        //                 vk::BufferUsageFlags::TRANSFER_SRC
        //                     | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
        //                     | vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR,
        //             )
        //             .build()
        //             .unwrap(),
        //             Some(&sbt_data),
        //         )?))
        //     };

        // let raygen = create_binding_table(0, desc.raygen_count)?;
        // let miss = create_binding_table(desc.raygen_count, desc.miss_count)?;
        // let hit = create_binding_table(desc.raygen_count + desc.miss_count, desc.hit_count)?;

        // Ok(Self {
        //     raygen: vk::StridedDeviceAddressRegionKHR {
        //         device_address: raygen
        //             .as_ref()
        //             .map(|b| Buffer::device_address(b))
        //             .unwrap_or(0),
        //         stride: prog_size as u64,
        //         size: (prog_size * desc.raygen_count as usize) as u64,
        //     },
        //     raygen_buf: raygen,
        //     miss: vk::StridedDeviceAddressRegionKHR {
        //         device_address: miss
        //             .as_ref()
        //             .map(|b| Buffer::device_address(b))
        //             .unwrap_or(0),
        //         stride: prog_size as u64,
        //         size: (prog_size * desc.miss_count as usize) as u64,
        //     },
        //     miss_buf: miss,
        //     hit: vk::StridedDeviceAddressRegionKHR {
        //         device_address: hit.as_ref().map(|b| Buffer::device_address(b)).unwrap_or(0),
        //         stride: prog_size as u64,
        //         size: (prog_size * desc.hit_count as usize) as u64,
        //     },
        //     hit_buf: hit,
        //     callable_buf: None,
        //     callable: vk::StridedDeviceAddressRegionKHR {
        //         device_address: Default::default(),
        //         stride: 0,
        //         size: 0,
        //     },
        // })
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct RayTraceShaderBindingsDesc {
    pub raygen_count: u32,
    pub hit_count: u32,
    pub miss_count: u32,
}
