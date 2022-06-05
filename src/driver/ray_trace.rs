use {
    super::{DescriptorBindingMap, Device, DriverError, PipelineDescriptorInfo, Shader},
    ash::vk,
    derive_builder::Builder,
    log::warn,
    std::{ffi::CString, ops::Deref, sync::Arc, thread::panicking},
};

#[derive(Debug)]
pub struct RayTracePipeline {
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,
    pub info: RayTracePipelineInfo,
    pub layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    shader_modules: Vec<vk::ShaderModule>,
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

        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();

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

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                layout,
                pipeline,
                shader_modules,
            })
        }
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
