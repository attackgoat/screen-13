use {
    super::{
        merge_push_constant_ranges, DescriptorBindingMap, Device, DriverError,
        PhysicalDeviceRayTracePipelineProperties, PipelineDescriptorInfo, Shader,
    },
    ash::vk,
    derive_builder::Builder,
    log::warn,
    std::{ffi::CString, ops::Deref, sync::Arc, thread::panicking},
};

/// Smart pointer handle to a [pipeline] object.
///
/// Also contains information about the object.
///
/// ## `Deref` behavior
///
/// `RayTracePipeline` automatically dereferences to [`vk::Pipeline`] (via the [`Deref`][deref]
/// trait), so you can call `vk::Pipeline`'s methods on a value of type `RayTracePipeline`. To avoid
/// name clashes with `vk::Pipeline`'s methods, the methods of `RayTracePipeline` itself are
/// associated functions, called using [fully qualified syntax]:
///
/// [pipeline]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPipeline.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
#[derive(Debug)]
pub struct RayTracePipeline {
    pub(crate) descriptor_bindings: DescriptorBindingMap,
    pub(crate) descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,
    pub info: RayTracePipelineInfo,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) push_constants: Vec<vk::PushConstantRange>,
    pipeline: vk::Pipeline,
    shader_modules: Vec<vk::ShaderModule>,
    shader_group_handles: Vec<u8>,
}

impl RayTracePipeline {
    /// Creates a new ray trace pipeline on the given device.
    ///
    /// The correct pipeline stages will be enabled based on the provided shaders. See [Shader] for
    /// details on all available stages.
    ///
    /// The number and composition of the `shader_groups` parameter must match the actual shaders
    /// provided.
    ///
    /// # Panics
    ///
    /// If shader code is not a multiple of four bytes.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::{RayTracePipeline, RayTracePipelineInfo, RayTraceShaderGroup, Shader};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build().unwrap())?);
    /// # let my_rgen_code = [0u8; 1];
    /// # let my_chit_code = [0u8; 1];
    /// # let my_miss_code = [0u8; 1];
    /// # let my_shadow_code = [0u8; 1];
    /// // shader code is raw SPIR-V code as bytes
    /// let info = RayTracePipelineInfo::new().max_ray_recursion_depth(1);
    /// let pipeline = RayTracePipeline::create(
    ///     &device,
    ///     info,
    ///     [
    ///         Shader::new_ray_gen(my_rgen_code.as_slice()),
    ///         Shader::new_closest_hit(my_chit_code.as_slice()),
    ///         Shader::new_miss(my_miss_code.as_slice()),
    ///         Shader::new_miss(my_shadow_code.as_slice()),
    ///     ],
    ///     [
    ///         RayTraceShaderGroup::new_general(0),
    ///         RayTraceShaderGroup::new_triangles(1, None),
    ///         RayTraceShaderGroup::new_general(2),
    ///         RayTraceShaderGroup::new_general(3),
    ///     ],
    /// )?;
    ///
    /// assert_ne!(*pipeline, vk::Pipeline::null());
    /// assert_eq!(pipeline.info.max_ray_recursion_depth, 1);
    /// # Ok(()) }
    /// ```
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

            merge_push_constant_ranges(&mut push_constants);

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

    /// Function returning a handle to a shader group of this pipeline.
    /// This can be used to construct a sbt.
    ///
    /// # Examples
    ///
    /// See
    /// [ray_trace.rs](https://github.com/attackgoat/screen-13/blob/master/examples/ray_trace.rs)
    /// for a detail example which constructs a shader binding table buffer using this function.
    pub fn group_handle(this: &Self, idx: usize) -> Result<&[u8], DriverError> {
        let &PhysicalDeviceRayTracePipelineProperties {
            shader_group_handle_size,
            ..
        } = this
            .device
            .ray_tracing_pipeline_properties
            .as_ref()
            .ok_or(DriverError::Unsupported)?;
        let start = idx * shader_group_handle_size as usize;
        let end = start + shader_group_handle_size as usize;

        Ok(&this.shader_group_handles[start..end])
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

/// Information used to create a [`RayTracePipeline`] instance.
#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Clone, Debug),
    pattern = "owned"
)]
pub struct RayTracePipelineInfo {
    /// The number of descriptors to allocate for a given binding when using bindless (unbounded)
    /// syntax.
    ///
    /// The default is `8192`.
    ///
    /// # Examples
    ///
    /// Basic usage (GLSL):
    ///
    /// ```glsl
    /// #version 460 core
    /// #extension GL_EXT_nonuniform_qualifier : require
    ///
    /// layout(set = 0, binding = 0, rgba8) readonly uniform image2D my_binding[];
    ///
    /// void main()
    /// {
    ///     // my_binding will have space for 8,192 images by default
    /// }
    /// ```
    #[builder(default = "8192")]
    pub bindless_descriptor_count: u32,

    /// The [maximum recursion depth] of shaders executed by this pipeline.
    ///
    /// [maximum recursion depth]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#ray-tracing-recursion-depth
    #[builder(default = "16")]
    pub max_ray_recursion_depth: u32,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,
}

impl RayTracePipelineInfo {
    /// Specifies a ray trace pipeline.
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

impl From<RayTracePipelineInfoBuilder> for RayTracePipelineInfo {
    fn from(info: RayTracePipelineInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl RayTracePipelineInfoBuilder {
    /// Builds a new `RayTracePipelineInfo`.
    pub fn build(self) -> RayTracePipelineInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

/// Describes the set of the shader stages to be included in each shader group in the ray trace
/// pipeline.
///
/// See
/// [VkRayTracingShaderGroupCreateInfoKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#VkRayTracingShaderGroupCreateInfoKHR).
#[derive(Debug)]
pub struct RayTraceShaderGroup {
    /// The optional index of the any-hit shader in the group if the shader group has type of
    /// [RayTraceShaderGroupType::TrianglesHitGroup] or
    /// [RayTraceShaderGroupType::ProceduralHitGroup].
    pub any_hit_shader: Option<u32>,

    /// The optional index of the closest hit shader in the group if the shader group has type of
    /// [RayTraceShaderGroupType::TrianglesHitGroup] or
    /// [RayTraceShaderGroupType::ProceduralHitGroup].
    pub closest_hit_shader: Option<u32>,

    /// The index of the ray generation, miss, or callable shader in the group if the shader group
    /// has type of [RayTraceShaderGroupType::General].
    pub general_shader: Option<u32>,

    /// The index of the intersection shader in the group if the shader group has type of
    /// [RayTraceShaderGroupType::ProceduralHitGroup].
    pub intersection_shader: Option<u32>,

    /// The type of hit group specified in this structure.
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

    /// Creates a new general-type shader group with the given general shader.
    pub fn new_general(general_shader: impl Into<Option<u32>>) -> Self {
        Self::new(
            RayTraceShaderGroupType::General,
            general_shader,
            None,
            None,
            None,
        )
    }

    /// Creates a new procedural-type shader group with the given intersection shader, and optional
    /// closest-hit and any-hit shaders.
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

    /// Creates a new triangles-type shader group with the given closest-hit shader and optional any-hit
    /// shader.
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

/// Describes a type of ray tracing shader group, which is a collection of shaders which run in the
/// specified mode.
#[derive(Debug)]
pub enum RayTraceShaderGroupType {
    /// A shader group with a general shader.
    General,

    /// A shader group with an intersection shader, and optional closest-hit and any-hit shaders.
    ProceduralHitGroup,

    /// A shader group with a closest-hit shader and optional any-hit shader.
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
