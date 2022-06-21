use std::cmp::Ordering;

use {
    super::{
        DescriptorBindingMap, Device, DriverError, PhysicalDeviceRayTracePipelineProperties,
        PipelineDescriptorInfo, Shader,
    },
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    std::{ffi::CString, ops::Deref, sync::Arc, thread::panicking},
};

#[derive(Debug)]
pub struct RayTracePipeline {
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,
    pub info: RayTracePipelineInfo,
    pub layout: vk::PipelineLayout,
    pub push_constants: Vec<vk::PushConstantRange>,
    pipeline: vk::Pipeline,
    shader_modules: Vec<vk::ShaderModule>,
    shader_group_handles: Vec<u8>,
}

impl RayTracePipeline {
    pub fn create<S>(
        device: &Arc<Device>,
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
        let group_count = shader_groups.len();

        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();
        let mut push_constants = shaders
            .iter()
            .map(|shader| shader.push_constant_range())
            .filter_map(|mut push_const| push_const.take())
            .collect::<Vec<_>>();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let mut descriptor_bindings = Shader::merge_descriptor_bindings(
            shaders
                .iter()
                .map(|shader| shader.descriptor_bindings(device)),
        );
        for (descriptor_info, _) in descriptor_bindings.values_mut() {
            if descriptor_info.binding_count() == 0 {
                descriptor_info.set_binding_count(info.bindless_descriptor_count);
            }
        }

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
                        .set_layouts(&descriptor_set_layout_handles)
                        .push_constant_ranges(&push_constants),
                    None,
                )
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
            let mut entry_points: Vec<CString> = Vec::with_capacity(shaders.len()); // Keep entry point names alive, since build() forgets references.
            let mut shader_stages: Vec<vk::PipelineShaderStageCreateInfo> =
                Vec::with_capacity(shaders.len());
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

            let mut specializations = Vec::with_capacity(shaders.len());
            let mut shader_modules = Vec::with_capacity(shaders.len());
            for shader in &shaders {
                let res = create_shader_module(shader);
                if res.is_err() {
                    device.destroy_pipeline_layout(layout, None);

                    for shader_module in &shader_modules {
                        device.destroy_shader_module(*shader_module, None);
                    }
                }

                let (module, entry_point) = res?;
                entry_points.push(CString::new(entry_point).unwrap());
                shader_modules.push(module);

                let mut stage = vk::PipelineShaderStageCreateInfo::builder()
                    .module(module)
                    .name(entry_points.last().unwrap().as_ref())
                    .stage(shader.stage);

                if let Some(spec_info) = &shader.specialization_info {
                    specializations.push(
                        vk::SpecializationInfo::builder()
                            .data(&spec_info.data)
                            .map_entries(&spec_info.map_entries)
                            .build(),
                    );
                    stage = stage.specialization_info(specializations.last().unwrap());
                }

                shader_stages.push(stage.build());
            }

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
                        .max_pipeline_ray_recursion_depth(
                            info.max_ray_recursion_depth.min(
                                device
                                    .ray_tracing_pipeline_properties
                                    .as_ref()
                                    .unwrap()
                                    .max_ray_recursion_depth,
                            ),
                        )
                        .layout(layout)
                        .build()],
                    None,
                )
                .map_err(|err| {
                    warn!("{err}");

                    device.destroy_pipeline_layout(layout, None);

                    for shader_module in &shader_modules {
                        device.destroy_shader_module(*shader_module, None);
                    }

                    DriverError::Unsupported
                })?[0];
            let device = Arc::clone(device);

            let &PhysicalDeviceRayTracePipelineProperties {
                shader_group_handle_size,
                ..
            } = device
                .ray_tracing_pipeline_properties
                .as_ref()
                .ok_or(DriverError::Unsupported)?;

            let ray_tracing_pipeline_ext = device
                .ray_tracing_pipeline_ext
                .as_ref()
                .ok_or(DriverError::Unsupported)?;

            if push_constants.len() > 1 {
                push_constants.sort_unstable_by(|lhs, rhs| match lhs.offset.cmp(&rhs.offset) {
                    Ordering::Equal => lhs.size.cmp(&rhs.size),
                    res => res,
                });

                let mut idx = 0;
                while idx + 1 < push_constants.len() {
                    let curr = push_constants[idx];
                    let next = push_constants[idx + 1];
                    let curr_end = curr.offset + curr.size;

                    // Check for overlapping push constant ranges; combine them and move the next
                    // one so it no longer overlaps
                    if curr_end > next.offset {
                        push_constants[idx].stage_flags |= next.stage_flags;

                        idx += 1;
                        push_constants[idx].offset = curr_end;
                        push_constants[idx].size -= curr_end - next.offset;
                    }

                    idx += 1;
                }

                for pcr in &push_constants {
                    trace!(
                        "effective push constants: {:?} {}..{}",
                        pcr.stage_flags,
                        pcr.offset,
                        pcr.offset + pcr.size
                    );
                }
            } else {
                for pcr in &push_constants {
                    trace!(
                        "detected push constants: {:?} {}..{}",
                        pcr.stage_flags,
                        pcr.offset,
                        pcr.offset + pcr.size
                    );
                }
            }

            // SAFETY:
            // According to [vulkan spec](https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/vkGetRayTracingShaderGroupHandlesKHR.html)
            // Valid usage of this function requires:
            // 1. pipeline must be raytracing pipeline.
            // 2. first_group must be less than the number of shader groups in the pipeline.
            // 3. the sum of first group and group_count must be less or equal to the number of shader
            //    modules in the pipeline.
            // 4. data_size must be at least shader_group_handle_size * group_count.
            // 5. pipeline must not have been created with VK_PIPELINE_CREATE_LIBRARY_BIT_KHR.
            //
            let shader_group_handles = {
                ray_tracing_pipeline_ext.get_ray_tracing_shader_group_handles(
                    pipeline,
                    0,
                    group_count as u32,
                    group_count * shader_group_handle_size as usize,
                )
            }
            .map_err(|_| DriverError::InvalidData)?;

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                layout,
                push_constants,
                pipeline,
                shader_modules,
                shader_group_handles,
            })
        }
    }
    ///
    /// Function returning a handle to a shader group of this pipeline.
    /// This can be used to construct a sbt.
    ///
    pub fn group_handle(&self, idx: usize) -> Result<&[u8], DriverError> {
        let &PhysicalDeviceRayTracePipelineProperties {
            shader_group_handle_size,
            ..
        } = self
            .device
            .ray_tracing_pipeline_properties
            .as_ref()
            .ok_or(DriverError::Unsupported)?;
        let start = idx * shader_group_handle_size as usize;
        let end = start + shader_group_handle_size as usize;
        Ok(&self.shader_group_handles[start..end])
    }
}

impl Deref for RayTracePipeline {
    type Target = vk::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl Drop for RayTracePipeline {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }

        for shader_module in self.shader_modules.drain(..) {
            unsafe {
                self.device.destroy_shader_module(shader_module, None);
            }
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
    #[builder(default = "8192")]
    pub bindless_descriptor_count: u32,

    #[builder(default = "16")]
    pub max_ray_recursion_depth: u32,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,
}

impl RayTracePipelineInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> RayTracePipelineInfoBuilder {
        Default::default()
    }
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
        closest_hit_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
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

    pub fn new_general(general_shader: impl Into<Option<u32>>) -> Self {
        Self::new(
            RayTraceShaderGroupType::General,
            general_shader,
            None,
            None,
            None,
        )
    }

    pub fn new_procedural(
        intersection_shader: u32,
        closest_hit_shader: impl Into<Option<u32>>,
        any_hit_shader: impl Into<Option<u32>>,
    ) -> Self {
        Self::new(
            RayTraceShaderGroupType::ProceduralHitGroup,
            None,
            intersection_shader,
            closest_hit_shader,
            any_hit_shader,
        )
    }

    pub fn new_triangles(closest_hit_shader: u32, any_hit_shader: impl Into<Option<u32>>) -> Self {
        Self::new(
            RayTraceShaderGroupType::TrianglesHitGroup,
            None,
            None,
            closest_hit_shader,
            any_hit_shader,
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
