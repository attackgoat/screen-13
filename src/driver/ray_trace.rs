use {
    super::{DescriptorBindingMap, Device, DriverError, PipelineDescriptorInfo, Shader},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    log::warn,
    std::{ffi::CString, ops::Deref, thread::panicking},
};

// #[derive(Debug)]
// pub struct ShaderBinding<P>
// where
//     P: SharedPointerKind,
// {
//     pub buffer: Option<Buffer<P>>,
//     pub region: vk::StridedDeviceAddressRegionKHR,
// }

// #[derive(Debug)]
// pub struct ShaderBindingTable<P>
// where
//     P: SharedPointerKind,
// {
//     pub ray_gen_buf: Option<Buffer<P>>,
//     pub ray_gen: vk::StridedDeviceAddressRegionKHR,
//     pub miss_buf: Option<Buffer<P>>,
//     pub miss: vk::StridedDeviceAddressRegionKHR,
//     pub hit_buf: Option<Buffer<P>>,
//     pub hit: vk::StridedDeviceAddressRegionKHR,
//     pub callable_buf: Option<Buffer<P>>,
//     pub callable: vk::StridedDeviceAddressRegionKHR,
// }

// impl<P> ShaderBindingTable<P>
// where
//     P: SharedPointerKind,
// {
//     fn create(
//         device: &SharedPointer<Device<P>, P>,
//         info: RayTraceShaderBindingsInfo,
//         pipeline: vk::Pipeline,
//     ) -> Result<ShaderBindingTable<P>, DriverError> {
//         let device = SharedPointer::clone(device);
//         let shader_group_handle_size = device
//             .ray_trace_pipeline_properties
//             .shader_group_handle_size as usize;
//         let group_count = info.raygen_count + info.miss_count + info.hit_count;
//         let group_handles_size = shader_group_handle_size * group_count as usize;
//         let group_handles: Vec<u8> = unsafe {
//             device
//                 .ray_tracing_pipeline_ext.as_ref().unwrap()
//                 .get_ray_tracing_shader_group_handles(pipeline, 0, group_count, group_handles_size)
//                 .map_err(|err| {warn!("{err}");DriverError::Unsupported})?
//         };
//         let prog_size = shader_group_handle_size;
//         let create_binding_table =
//             |entry_offset: u32, entry_count: u32| -> Result<Option<Buffer<P>>, DriverError> {
//                 if entry_count == 0 {
//                     return Ok(None);
//                 }

//                 let mut sbt_data = vec![0u8; entry_count as usize * prog_size];

//                 for dst in 0..entry_count as usize {
//                     let src = dst + entry_offset as usize;
//                     sbt_data[dst * prog_size..dst * prog_size + shader_group_handle_size]
//                         .copy_from_slice(
//                             &group_handles[src * shader_group_handle_size
//                                 ..src * shader_group_handle_size + shader_group_handle_size],
//                         );
//                 }

//                 Ok(Some(Buffer::create_with_data(
//                     &device,
//                     BufferDesc::new(
//                         sbt_data.len() ,
//                         vk::BufferUsageFlags::TRANSFER_SRC
//                             | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
//                             | vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR,
//                     )
//                     .build()
//                     .unwrap(),
//                     Some(&sbt_data),
//                 )?))
//             };

//         let raygen = create_binding_table(0, info.raygen_count)?;
//         let miss = create_binding_table(info.raygen_count, info.miss_count)?;
//         let hit = create_binding_table(info.raygen_count + info.miss_count, info.hit_count)?;

//         Ok(Self {
//             raygen: vk::StridedDeviceAddressRegionKHR {
//                 device_address: raygen
//                     .as_ref()
//                     .map(|b| Buffer::device_address(b))
//                     .unwrap_or(0),
//                 stride: prog_size ,
//                 size: (prog_size * info.raygen_count as usize) as vk::DeviceSize ,
//             },
//             raygen_buf: raygen,
//             miss: vk::StridedDeviceAddressRegionKHR {
//                 device_address: miss
//                     .as_ref()
//                     .map(|b| Buffer::device_address(b))
//                     .unwrap_or(0),
//                 stride: prog_size,
//                 size: (prog_size * info.miss_count as usize) as vk::DeviceSize  ,
//             },
//             miss_buf: miss,
//             hit: vk::StridedDeviceAddressRegionKHR {
//                 device_address: hit.as_ref().map(|b| Buffer::device_address(b)).unwrap_or(0),
//                 stride: prog_size ,
//                 size: (prog_size * info.hit_count as usize) as vk::DeviceSize  ,
//             },
//             hit_buf: hit,
//             callable_buf: None,
//             callable: vk::StridedDeviceAddressRegionKHR {
//                 device_address: Default::default(),
//                 stride: 0,
//                 size: 0,
//             },
//         })
//     }
// }

// #[derive(Clone, Debug)]
// pub struct RayTraceShaderBindingsInfo {
//     pub raygen_count: u32,
//     pub hit_count: u32,
//     pub miss_count: u32,
// }

#[derive(Debug)]
pub struct RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo<P>,
    device: SharedPointer<Device<P>, P>,
    pub info: RayTracePipelineInfo,
    pub layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl<P> RayTracePipeline<P>
where
    P: SharedPointerKind,
{
    pub fn create<S>(
        device: &SharedPointer<Device<P>, P>,
        info: impl Into<RayTracePipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
        shader_groups: impl IntoIterator<Item = RayTraceShaderGroup>,
    ) -> Result<Self, DriverError>
    where
        S: Into<Shader>,
    {
        let info = info.into();
        let shader_groups = shader_groups
            .into_iter()
            .map(|shader_group| shader_group.into())
            .collect::<Vec<_>>();

        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let descriptor_bindings = Shader::merge_descriptor_bindings(
            shaders
                .iter()
                .map(|shader| shader.descriptor_bindings(device)),
        );

        let descriptor_info = PipelineDescriptorInfo::create(device, &descriptor_bindings)?;
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
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
            let mut entry_points: Vec<CString> = Vec::with_capacity(shaders.len()); // Keep entry point names alive, since build() forgets references.
            let mut shader_stages: Vec<vk::PipelineShaderStageCreateInfo> =
                Vec::with_capacity(shaders.len());
            let mut prev_stage: Option<vk::ShaderStageFlags> = None;
            let mut raygen_entry_count = 0;
            let mut miss_entry_count = 0;
            let create_shader_module =
                |info: &Shader| -> Result<(vk::ShaderModule, String), DriverError> {
                    let shader_module_create_info = vk::ShaderModuleCreateInfo {
                        code_size: info.spirv.len(),
                        p_code: info.spirv.as_ptr() as *const u32,
                        ..Default::default()
                    };
                    let shader_module = device
                        .create_shader_module(&shader_module_create_info, None)
                        .map_err(|err| {
                            warn!("{err}");

                            DriverError::Unsupported
                        })?;

                    Ok((shader_module, info.entry_name.clone()))
                };

            for shader in &shaders {
                let (module, entry_point) = create_shader_module(shader)?;
                entry_points.push(CString::new(entry_point).unwrap());

                let mut stage = vk::PipelineShaderStageCreateInfo::builder()
                    .module(module)
                    .name(entry_points.last().unwrap().as_ref());

                match shader.stage {
                    vk::ShaderStageFlags::RAYGEN_KHR => {
                        assert!(
                            prev_stage == None
                                || prev_stage == Some(vk::ShaderStageFlags::RAYGEN_KHR)
                        );

                        raygen_entry_count += 1;
                        stage = stage.stage(vk::ShaderStageFlags::RAYGEN_KHR);
                    }
                    vk::ShaderStageFlags::MISS_KHR => {
                        assert!(
                            prev_stage == Some(vk::ShaderStageFlags::RAYGEN_KHR)
                                || prev_stage == Some(vk::ShaderStageFlags::MISS_KHR)
                        );

                        miss_entry_count += 1;
                        stage = stage.stage(vk::ShaderStageFlags::MISS_KHR);
                    }
                    vk::ShaderStageFlags::CLOSEST_HIT_KHR => {
                        assert!(
                            prev_stage == Some(vk::ShaderStageFlags::MISS_KHR)
                                || prev_stage == Some(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                        );

                        stage = stage.stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR);
                    }
                    _ => unimplemented!(),
                }

                shader_stages.push(stage.build());

                prev_stage = Some(shader.stage);
            }

            assert!(raygen_entry_count > 0);
            assert!(miss_entry_count > 0);

            let pipeline = device
                .ray_tracing_pipeline_ext
                .as_ref()
                .unwrap()
                .create_ray_tracing_pipelines(
                    vk::DeferredOperationKHR::null(),
                    vk::PipelineCache::null(),
                    &[vk::RayTracingPipelineCreateInfoKHR::builder()
                        .stages(&shader_stages)
                        .groups(&shader_groups)
                        .max_pipeline_ray_recursion_depth(info.max_ray_recursion_depth) // TODO
                        .layout(layout)
                        .build()],
                    None,
                )
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?[0];
            let device = SharedPointer::clone(device);

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                layout,
                pipeline,
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

#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Clone, Debug),
    pattern = "owned"
)]
pub struct RayTracePipelineInfo {
    #[builder(default = "16")]
    pub max_ray_recursion_depth: u32,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,
}

impl Default for RayTracePipelineInfo {
    fn default() -> Self {
        RayTracePipelineInfoBuilder::default().build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl RayTracePipelineInfoBuilder {
    pub fn build(self) -> RayTracePipelineInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
pub struct RayTraceShaderGroup {
    pub any_hit_shader: Option<u32>,
    pub closest_hit_shader: Option<u32>,
    pub general_shader: Option<u32>,
    pub intersection_shader: Option<u32>,
    pub ty: RayTraceShaderGroupType,
}

impl RayTraceShaderGroup {
    fn new(
        ty: RayTraceShaderGroupType,
        general_shader: impl Into<Option<u32>>,
        intersection_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
        closest_hit_shader: impl Into<Option<u32>>,
    ) -> Self {
        let any_hit_shader = any_hit_shader.into();
        let closest_hit_shader = closest_hit_shader.into();
        let general_shader = general_shader.into();
        let intersection_shader = intersection_shader.into();

        Self {
            any_hit_shader,
            closest_hit_shader,
            general_shader,
            intersection_shader,
            ty,
        }
    }

    pub fn new_general(
        general_shader: impl Into<Option<u32>>,
        intersection_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
        closest_hit_shader: impl Into<Option<u32>>,
    ) -> Self {
        Self::new(
            RayTraceShaderGroupType::General,
            general_shader,
            intersection_shader,
            any_hit_shader,
            closest_hit_shader,
        )
    }

    pub fn new_procedural(
        general_shader: impl Into<Option<u32>>,
        intersection_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
        closest_hit_shader: impl Into<Option<u32>>,
    ) -> Self {
        Self::new(
            RayTraceShaderGroupType::ProceduralHitGroup,
            general_shader,
            intersection_shader,
            any_hit_shader,
            closest_hit_shader,
        )
    }

    pub fn new_triangles(
        general_shader: impl Into<Option<u32>>,
        intersection_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
        closest_hit_shader: impl Into<Option<u32>>,
    ) -> Self {
        Self::new(
            RayTraceShaderGroupType::TrianglesHitGroup,
            general_shader,
            intersection_shader,
            any_hit_shader,
            closest_hit_shader,
        )
    }
}

impl From<RayTraceShaderGroup> for vk::RayTracingShaderGroupCreateInfoKHR {
    fn from(shader_group: RayTraceShaderGroup) -> Self {
        vk::RayTracingShaderGroupCreateInfoKHR::builder()
            .ty(shader_group.ty.into())
            .any_hit_shader(shader_group.any_hit_shader.unwrap_or(vk::SHADER_UNUSED_KHR))
            .closest_hit_shader(
                shader_group
                    .closest_hit_shader
                    .unwrap_or(vk::SHADER_UNUSED_KHR),
            )
            .general_shader(shader_group.general_shader.unwrap_or(vk::SHADER_UNUSED_KHR))
            .intersection_shader(
                shader_group
                    .intersection_shader
                    .unwrap_or(vk::SHADER_UNUSED_KHR),
            )
            .build()
    }
}

#[derive(Debug)]
pub enum RayTraceShaderGroupType {
    General,
    ProceduralHitGroup,
    TrianglesHitGroup,
}

impl From<RayTraceShaderGroupType> for vk::RayTracingShaderGroupTypeKHR {
    fn from(ty: RayTraceShaderGroupType) -> Self {
        match ty {
            RayTraceShaderGroupType::General => vk::RayTracingShaderGroupTypeKHR::GENERAL,
            RayTraceShaderGroupType::ProceduralHitGroup => {
                vk::RayTracingShaderGroupTypeKHR::PROCEDURAL_HIT_GROUP
            }
            RayTraceShaderGroupType::TrianglesHitGroup => {
                vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP
            }
        }
    }
}
