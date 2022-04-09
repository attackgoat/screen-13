use {
    super::{
        DescriptorBindingMap, DescriptorSetLayout, Device, DriverError, PipelineDescriptorInfo,
        SampleCount, Shader, SpecializationInfo,
    },
    crate::{as_u32_slice, ptr::Shared},
    anyhow::Context,
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    log::trace,
    ordered_float::OrderedFloat,
    std::{collections::BTreeMap, ffi::CString, thread::panicking},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendMode {
    Alpha,
    Replace,
}

impl BlendMode {
    pub fn into_vk(&self) -> vk::PipelineColorBlendAttachmentState {
        match self {
            Self::Alpha => vk::PipelineColorBlendAttachmentState {
                blend_enable: 1,
                src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            },
            Self::Replace => vk::PipelineColorBlendAttachmentState {
                blend_enable: 0,
                src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ZERO,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            },
        }
    }
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Replace
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DepthStencilMode {
    pub back: StencilMode,
    pub bounds_test: bool,
    pub compare_op: vk::CompareOp,
    pub depth_test: bool,
    pub depth_write: bool,
    pub front: StencilMode,
    pub min: OrderedFloat<f32>,
    pub max: OrderedFloat<f32>,
    pub stencil_test: bool,
}

impl DepthStencilMode {
    pub(super) fn into_vk(self) -> vk::PipelineDepthStencilStateCreateInfo {
        vk::PipelineDepthStencilStateCreateInfo {
            back: self.back.into_vk(),
            depth_bounds_test_enable: self.bounds_test as _,
            depth_compare_op: self.compare_op,
            depth_test_enable: self.depth_test as _,
            depth_write_enable: self.depth_write as _,
            front: self.front.into_vk(),
            max_depth_bounds: *self.max,
            min_depth_bounds: *self.min,
            stencil_test_enable: self.stencil_test as _,
            ..Default::default()
        }
    }
}

impl Default for DepthStencilMode {
    fn default() -> Self {
        Self {
            back: StencilMode::Noop,
            bounds_test: false,
            compare_op: vk::CompareOp::GREATER_OR_EQUAL,
            depth_test: true,
            depth_write: true,
            front: StencilMode::Noop,
            min: OrderedFloat(0.0),
            max: OrderedFloat(1.0),
            stencil_test: false,
        }
    }
}

#[derive(Debug)]
pub struct GraphicPipeline<P>
where
    P: SharedPointerKind,
{
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo<P>,
    device: Shared<Device<P>, P>,
    pub info: GraphicPipelineInfo,
    pub layout: vk::PipelineLayout,
    pub push_constant_ranges: Vec<vk::PushConstantRange>,
    shader_modules: Vec<vk::ShaderModule>,
    pub state: GraphicPipelineState,
}

impl<P> GraphicPipeline<P>
where
    P: SharedPointerKind,
{
    pub fn create<S>(
        device: &Shared<Device<P>, P>,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Result<Self, DriverError>
    where
        S: Into<Shader>,
    {
        trace!("create");

        let device = Shared::clone(device);
        let info = info.into();
        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();

        // Use SPIR-V reflection to get the types and counts of all descriptors
        let mut descriptor_bindings = shaders
            .iter()
            .map(|shader| shader.descriptor_bindings(&device))
            .collect::<Result<Vec<_>, _>>()?;

        // We allow extra descriptors because specialization constants aren't specified yet
        if let Some(extra_descriptors) = &info.extra_descriptors {
            descriptor_bindings.push(extra_descriptors.clone());
        }

        let descriptor_bindings = Shader::merge_descriptor_bindings(descriptor_bindings);
        let stages = shaders
            .iter()
            .map(|shader| shader.stage)
            .reduce(|j, k| j | k)
            .unwrap_or_default();
        let descriptor_info =
            PipelineDescriptorInfo::create(&device, &descriptor_bindings, stages)?;
        let descriptor_sets_layouts = descriptor_info
            .layouts
            .iter()
            .map(|(_, descriptor_set_layout)| **descriptor_set_layout)
            .collect::<Box<[_]>>();

        let push_constant_ranges = shaders
            .iter()
            .map(|shader| shader.push_constant_range())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|mut push_const| push_const.take())
            .collect::<Vec<_>>();

        // for pcr in &push_constant_ranges {
        //     trace!("Graphic push constant {:?} {}..{}", pcr.stage_flags, pcr.offset, pcr.offset + pcr.size);
        // }

        unsafe {
            let layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .set_layouts(&descriptor_sets_layouts)
                        .push_constant_ranges(&push_constant_ranges),
                    None,
                )
                .map_err(|_| DriverError::Unsupported)?;
            let shader_info = shaders
                .into_iter()
                .map(|shader| {
                    let shader_module_create_info = vk::ShaderModuleCreateInfo {
                        code_size: shader.spirv.len(),
                        p_code: shader.spirv.as_ptr() as *const u32,
                        ..Default::default()
                    };
                    let shader_module = device
                        .create_shader_module(&shader_module_create_info, None)
                        .map_err(|_| DriverError::Unsupported)?;
                    let shader_stage = Stage {
                        flags: shader.stage,
                        module: shader_module,
                        name: CString::new(shader.entry_name.as_str()).unwrap(),
                        specialization_info: shader.specialization_info,
                    };

                    Result::<_, DriverError>::Ok((shader_module, shader_stage))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut shader_modules = vec![];
            let mut stages = vec![];
            shader_info
                .into_iter()
                .for_each(|(shader_module, shader_stage)| {
                    shader_modules.push(shader_module);
                    stages.push(shader_stage);
                });

            let vertex_input = VertexInputState {
                vertex_attribute_descriptions: match info.vertex_input {
                    VertexInputMode::BitmapFont => vec![
                        vk::VertexInputAttributeDescription {
                            location: 0,
                            binding: 0,
                            format: vk::Format::R32G32_SFLOAT,
                            offset: 0,
                        },
                        vk::VertexInputAttributeDescription {
                            location: 1,
                            binding: 0,
                            format: vk::Format::R32G32_SFLOAT,
                            offset: 8,
                        },
                        vk::VertexInputAttributeDescription {
                            location: 2,
                            binding: 0,
                            format: vk::Format::R32_SINT,
                            offset: 16,
                        },
                    ],
                    VertexInputMode::ImGui => vec![
                        vk::VertexInputAttributeDescription {
                            location: 0,
                            binding: 0,
                            format: vk::Format::R32G32_SFLOAT,
                            offset: 0,
                        },
                        vk::VertexInputAttributeDescription {
                            location: 1,
                            binding: 0,
                            format: vk::Format::R32G32_SFLOAT,
                            offset: 8,
                        },
                        vk::VertexInputAttributeDescription {
                            location: 2,
                            binding: 0,
                            format: vk::Format::R8G8B8A8_UNORM,
                            offset: 16,
                        },
                    ],
                    VertexInputMode::StaticMesh => vec![],
                },
                vertex_binding_descriptions: match info.vertex_input {
                    VertexInputMode::BitmapFont => vec![vk::VertexInputBindingDescription {
                        binding: 0,
                        stride: 20,
                        input_rate: vk::VertexInputRate::VERTEX,
                    }],
                    VertexInputMode::ImGui => vec![vk::VertexInputBindingDescription {
                        binding: 0,
                        stride: 20,
                        input_rate: vk::VertexInputRate::VERTEX,
                    }],
                    VertexInputMode::StaticMesh => vec![],
                },
            };
            let rasterization = RasterizationState {
                vertex_input: info.vertex_input,
                two_sided: info.two_sided,
            };
            let multisample = MultisampleState {
                rasterization_samples: match info.vertex_input {
                    VertexInputMode::BitmapFont | VertexInputMode::ImGui => SampleCount::X1,
                    VertexInputMode::StaticMesh => info.samples,
                },
                ..Default::default()
            };

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                layout,
                push_constant_ranges,
                shader_modules,
                state: GraphicPipelineState {
                    layout,
                    multisample,
                    rasterization,
                    stages,
                    vertex_input,
                },
            })
        }
    }
}

impl<P> Drop for GraphicPipeline<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device.destroy_pipeline_layout(self.layout, None);
        }

        for shader_module in self.shader_modules.drain(..) {
            unsafe {
                self.device.destroy_shader_module(shader_module, None);
            }
        }
    }
}

#[derive(Builder, Clone, Debug, Default, PartialEq)]
#[builder(pattern = "owned")]
pub struct GraphicPipelineInfo {
    #[builder(default)]
    pub blend: BlendMode,
    #[builder(default)]
    pub depth_stencil: Option<DepthStencilMode>,
    /// A map of extra descriptors not directly specified in the shader SPIR-V code.
    ///
    /// Use this for specialization constants, as they will not appear in the automatic descriptor
    /// binding map.
    #[builder(default, setter(strip_option))]
    pub extra_descriptors: Option<DescriptorBindingMap>,
    #[builder(default = "SampleCount::X1")]
    pub samples: SampleCount,
    #[builder(default)]
    pub two_sided: bool,
    #[builder(default)]
    pub vertex_input: VertexInputMode,
}

impl GraphicPipelineInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> GraphicPipelineInfoBuilder {
        GraphicPipelineInfoBuilder::default()
    }
}

impl From<GraphicPipelineInfoBuilder> for GraphicPipelineInfo {
    fn from(info: GraphicPipelineInfoBuilder) -> Self {
        info.build().unwrap()
    }
}

#[derive(Debug)]
pub struct GraphicPipelineState {
    pub layout: vk::PipelineLayout,
    pub multisample: MultisampleState,
    pub rasterization: RasterizationState,
    pub stages: Vec<Stage>,
    pub vertex_input: VertexInputState,
}

#[derive(Debug, Default)]
pub struct MultisampleState {
    pub alpha_to_coverage_enable: bool,
    pub alpha_to_one_enable: bool,
    pub flags: vk::PipelineMultisampleStateCreateFlags,
    pub min_sample_shading: f32,
    pub rasterization_samples: SampleCount,
    pub sample_mask: Vec<u32>,
    pub sample_shading_enable: bool,
}

#[derive(Debug, Default)]
pub struct RasterizationState {
    pub vertex_input: VertexInputMode,
    pub two_sided: bool,
}

#[derive(Debug)]
pub struct Stage {
    pub flags: vk::ShaderStageFlags,
    pub module: vk::ShaderModule,
    pub name: CString,
    pub specialization_info: Option<SpecializationInfo>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StencilMode {
    Noop, // TODO: Provide some sensible modes
}

impl StencilMode {
    fn into_vk(self) -> vk::StencilOpState {
        match self {
            Self::Noop => vk::StencilOpState {
                fail_op: vk::StencilOp::KEEP,
                pass_op: vk::StencilOp::KEEP,
                depth_fail_op: vk::StencilOp::KEEP,
                compare_op: vk::CompareOp::ALWAYS,
                ..Default::default()
            },
        }
    }
}

impl Default for StencilMode {
    fn default() -> Self {
        Self::Noop
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VertexInputMode {
    // AnimatedMesh,
    BitmapFont,
    ImGui,
    StaticMesh,
}

impl Default for VertexInputMode {
    fn default() -> Self {
        Self::StaticMesh
    }
}

#[derive(Debug)]
pub struct VertexInputState {
    pub vertex_binding_descriptions: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attribute_descriptions: Vec<vk::VertexInputAttributeDescription>,
}
