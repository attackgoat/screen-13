use {
    super::{
        shader::ShaderCode, DescriptorBindingMap, Device, DriverError, PipelineDescriptorInfo,
        Shader, SpecializationInfo,
    },
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    std::{ffi::CString, ops::Deref, thread::panicking},
};

#[derive(Debug)]
pub struct ComputePipeline<P>
where
    P: SharedPointerKind,
{
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo<P>,
    pub device: SharedPointer<Device<P>, P>,
    pub layout: vk::PipelineLayout,
    pub info: ComputePipelineInfo,
    pipeline: vk::Pipeline,
    pub push_constants: Option<vk::PushConstantRange>,
}

impl<P> ComputePipeline<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &SharedPointer<Device<P>, P>,
        info: impl Into<ComputePipelineInfo>,
    ) -> Result<Self, DriverError> {
        use std::slice::from_ref;

        trace!("create");

        let device = SharedPointer::clone(device);
        let info: ComputePipelineInfo = info.into();
        let shader = info.clone().into_shader();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let descriptor_bindings = shader.descriptor_bindings(&device);
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

impl<P> Deref for ComputePipeline<P>
where
    P: SharedPointerKind,
{
    type Target = vk::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl<P> Drop for ComputePipeline<P>
where
    P: SharedPointerKind,
{
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

#[derive(Builder, Clone, Debug)]
#[builder(pattern = "owned")]
pub struct ComputePipelineInfo {
    /// The GLSL or HLSL shader entry point name, or `main` by default.
    #[builder(setter(strip_option), default = "String::from(\"main\")")]
    pub entry_name: String,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,

    /// Data about Vulkan specialization constants.
    #[builder(default, setter(strip_option))]
    pub specialization_info: Option<SpecializationInfo>,

    /// Shader code.
    pub spirv: Vec<u8>,
}

impl ComputePipelineInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(spirv: impl ShaderCode) -> ComputePipelineInfoBuilder {
        ComputePipelineInfoBuilder::default().spirv(spirv.into_vec())
    }

    pub fn into_shader(self) -> Shader {
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
        Self::new(spirv).build().unwrap()
    }
}

impl From<ComputePipelineInfoBuilder> for ComputePipelineInfo {
    fn from(info: ComputePipelineInfoBuilder) -> Self {
        info.build().unwrap()
    }
}
