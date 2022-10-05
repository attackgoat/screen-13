//! Computing pipeline types

use {
    super::{
        shader::{
            DescriptorBindingMap, PipelineDescriptorInfo, Shader, ShaderCode, SpecializationInfo,
        },
        Device, DriverError,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::{trace, warn},
    std::{ffi::CString, ops::Deref, sync::Arc, thread::panicking},
};

/// Smart pointer handle to a [pipeline] object.
///
/// Also contains information about the object.
///
/// ## `Deref` behavior
///
/// `ComputePipeline` automatically dereferences to [`vk::Pipeline`] (via the [`Deref`][deref]
/// trait), so you can call `vk::Pipeline`'s methods on a value of type `ComputePipeline`.
///
/// [pipeline]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPipeline.html
/// [deref]: core::ops::Deref
#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) descriptor_bindings: DescriptorBindingMap,
    pub(crate) descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,
    pub(crate) layout: vk::PipelineLayout,

    /// Information used to create this object.
    pub info: ComputePipelineInfo,

    pipeline: vk::Pipeline,
    pub(crate) push_constants: Option<vk::PushConstantRange>,
}

impl ComputePipeline {
    /// Creates a new compute pipeline on the given device.
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
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_shader_code = [0u8; 1];
    /// // my_shader_code is raw SPIR-V code as bytes
    /// let info = ComputePipelineInfo::new(my_shader_code.as_slice());
    /// let pipeline = ComputePipeline::create(&device, info)?;
    ///
    /// assert_ne!(*pipeline, vk::Pipeline::null());
    /// assert_eq!(pipeline.info.entry_name.as_str(), "main");
    /// # Ok(()) }
    /// ```
    pub fn create(
        device: &Arc<Device>,
        info: impl Into<ComputePipelineInfo>,
    ) -> Result<Self, DriverError> {
        use std::slice::from_ref;

        trace!("create");

        let device = Arc::clone(device);
        let info: ComputePipelineInfo = info.into();
        let shader = info.clone().into_shader();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let mut descriptor_bindings = shader.descriptor_bindings(&device);
        for (descriptor_info, _) in descriptor_bindings.values_mut() {
            if descriptor_info.binding_count() == 0 {
                descriptor_info.set_binding_count(info.bindless_descriptor_count);
            }
        }

        let descriptor_info = PipelineDescriptorInfo::create(&device, &descriptor_bindings)?;
        let descriptor_set_layouts = descriptor_info
            .layouts
            .iter()
            .map(|(_, descriptor_set_layout)| **descriptor_set_layout)
            .collect::<Box<[_]>>();

        unsafe {
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
            let entry_name = CString::new(info.entry_name.as_bytes()).unwrap();
            let mut stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
                .module(shader_module)
                .stage(shader.stage)
                .name(&entry_name);
            let specialization_info = info.specialization_info.as_ref().map(|info| {
                vk::SpecializationInfo::builder()
                    .map_entries(&info.map_entries)
                    .data(&info.data)
                    .build()
            });

            if let Some(specialization_info) = &specialization_info {
                stage_create_info = stage_create_info.specialization_info(specialization_info);
            }

            let mut layout_info =
                vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts);

            let push_constants = shader.push_constant_range();
            if let Some(push_constants) = &push_constants {
                layout_info = layout_info.push_constant_ranges(from_ref(push_constants));
            }

            let layout = device
                .create_pipeline_layout(&layout_info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
            let pipeline_info = vk::ComputePipelineCreateInfo::builder()
                .stage(stage_create_info.build())
                .layout(layout);
            let pipeline = device
                .create_compute_pipelines(
                    vk::PipelineCache::null(),
                    from_ref(&pipeline_info.build()),
                    None,
                )
                .map_err(|(_, err)| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?[0];

            device.destroy_shader_module(shader_module, None);

            Ok(ComputePipeline {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                layout,
                pipeline,
                push_constants,
            })
        }
    }
}

impl Deref for ComputePipeline {
    type Target = vk::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}

/// Information used to create a [`ComputePipeline`] instance.
#[derive(Builder, Clone, Debug)]
#[builder(
    pattern = "owned",
    build_fn(
        private,
        name = "fallible_build",
        error = "ComputePipelineInfoBuilderError"
    )
)]
pub struct ComputePipelineInfo {
    /// The number of descriptors to allocate for a given binding when using bindless (unbounded)
    /// syntax.
    ///
    /// The default is `8192`.
    ///
    /// # Examples
    ///
    /// Basic usage (GLSL):
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 460 core
    /// #extension GL_EXT_nonuniform_qualifier : require
    ///
    /// layout(set = 0, binding = 0, rgba8) writeonly uniform image2D my_binding[];
    ///
    /// void main()
    /// {
    ///     // my_binding will have space for 8,192 images by default
    /// }
    /// # "#, comp);
    /// ```
    #[builder(default = "8192")]
    pub bindless_descriptor_count: u32,

    /// The GLSL or HLSL shader entry point name, or `main` by default.
    #[builder(setter(strip_option), default = "String::from(\"main\")")]
    pub entry_name: String,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,

    /// Data about Vulkan specialization constants.
    ///
    /// # Examples
    ///
    /// Basic usage (GLSL):
    ///
    /// ```
    /// # inline_spirv::inline_spirv!(r#"
    /// #version 460 core
    ///
    /// // Defaults to 6 if not set using ComputePipelineInfo.specialization_info!
    /// layout(constant_id = 0) const uint MY_COUNT = 6;
    ///
    /// layout(set = 0, binding = 0) uniform sampler2D my_samplers[MY_COUNT];
    ///
    /// void main()
    /// {
    ///     // Code uses MY_COUNT number of my_samplers here
    /// }
    /// # "#, comp);
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::compute::{ComputePipeline, ComputePipelineInfo};
    /// # use screen_13::driver::shader::{SpecializationInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_shader_code = [0u8; 1];
    /// // We instead specify 42 for MY_COUNT:
    /// let info = ComputePipelineInfo::new(my_shader_code.as_slice())
    ///     .specialization_info(SpecializationInfo::new(
    ///         [vk::SpecializationMapEntry {
    ///             constant_id: 0,
    ///             offset: 0,
    ///             size: 4,
    ///         }],
    ///         42u32.to_ne_bytes()
    ///     ));
    /// let pipeline = ComputePipeline::create(&device, info)?;
    /// # Ok(()) }
    /// ```
    #[builder(default, setter(strip_option))]
    pub specialization_info: Option<SpecializationInfo>,

    /// Shader code.
    ///
    /// Although SPIR-V code is specified as `u32` values, this field uses `u8` in order to make
    /// loading from file simpler. You should always have a SPIR-V code length which is a multiple
    /// of four bytes, or a panic will happen during [`ComputePipeline::create`].
    pub spirv: Vec<u8>,
}

impl ComputePipelineInfo {
    /// Specifies a compute pipeline with the given shader code.
    ///
    /// # Panics
    ///
    /// If shader code is not a multiple of four bytes.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(spirv: impl ShaderCode) -> ComputePipelineInfoBuilder {
        ComputePipelineInfoBuilder::default().spirv(spirv.into_vec())
    }

    fn into_shader(self) -> Shader {
        let mut shader =
            Shader::new(vk::ShaderStageFlags::COMPUTE, self.spirv).entry_name(self.entry_name);

        if let Some(specialization_info) = self.specialization_info {
            shader = shader.specialization_info(specialization_info);
        }

        shader.build()
    }
}

impl<S> From<S> for ComputePipelineInfo
where
    S: ShaderCode,
{
    fn from(spirv: S) -> Self {
        Self::new(spirv).build()
    }
}

impl From<ComputePipelineInfoBuilder> for ComputePipelineInfo {
    fn from(info: ComputePipelineInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl ComputePipelineInfoBuilder {
    /// Builds a new `ComputePipelineInfo`.
    pub fn build(self) -> ComputePipelineInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
struct ComputePipelineInfoBuilderError;

impl From<UninitializedFieldError> for ComputePipelineInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}
