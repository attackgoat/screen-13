use {
    super::{
        DescriptorBindingMap, Device, DriverError, PipelineDescriptorInfo, SampleCount, Shader,
        SpecializationInfo,
    },
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    ordered_float::OrderedFloat,
    std::{ffi::CString, thread::panicking},
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

        let vertex_input = shaders
            .iter()
            .find(|shader| shader.stage == vk::ShaderStageFlags::VERTEX)
            .expect("Unable to find vertex shader")
            .vertex_input()?;

        // Check for proper stages because vulkan may not complain but this is bad
        let has_fragment_stage = shaders
            .iter()
            .find(|shader| shader.stage.contains(vk::ShaderStageFlags::FRAGMENT))
            .is_some();
        let has_tesselation_stage = shaders
            .iter()
            .find(|shader| {
                shader
                    .stage
                    .contains(vk::ShaderStageFlags::TESSELLATION_CONTROL)
            })
            .is_some()
            && shaders
                .iter()
                .find(|shader| {
                    shader
                        .stage
                        .contains(vk::ShaderStageFlags::TESSELLATION_EVALUATION)
                })
                .is_some();
        let has_geometry_stage = shaders
            .iter()
            .find(|shader| shader.stage.contains(vk::ShaderStageFlags::GEOMETRY))
            .is_some();

        debug_assert!(
            has_fragment_stage || has_tesselation_stage || has_geometry_stage,
            "invalid shader stage combination"
        );

        let descriptor_bindings = shaders
            .iter()
            .map(|shader| shader.descriptor_bindings(&device))
            .collect::<Vec<_>>();
        if let Some(err) = descriptor_bindings.iter().find(|item| item.is_err()) {
            warn!("Unable to inspect shader descriptor bindings: {:?}", err);

            return Err(DriverError::Unsupported);
        }

        let descriptor_bindings = Shader::merge_descriptor_bindings(
            descriptor_bindings.into_iter().map(|item| item.unwrap()),
        );
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

        for pcr in &push_constant_ranges {
            trace!(
                "detected push constants: {:?} {}..{}",
                pcr.stage_flags,
                pcr.offset,
                pcr.offset + pcr.size
            );
        }

        unsafe {
            let layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .set_layouts(&descriptor_sets_layouts)
                        .push_constant_ranges(&push_constant_ranges),
                    None,
                )
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
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
                        .map_err(|err| {
                            warn!("{err}");

                            DriverError::Unsupported
                        })?;
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

            let rasterization = RasterizationState {
                two_sided: info.two_sided,
            };
            let multisample = MultisampleState {
                rasterization_samples: info.samples,
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
    #[builder(default = "vk::CullModeFlags::BACK")]
    pub cull_mode: vk::CullModeFlags,
    #[builder(default, setter(strip_option))]
    pub depth_stencil: Option<DepthStencilMode>,
    #[builder(default = "vk::FrontFace::COUNTER_CLOCKWISE")]
    pub front_face: vk::FrontFace,
    #[builder(default = "vk::PolygonMode::FILL")]
    pub polygon_mode: vk::PolygonMode,
    #[builder(default = "SampleCount::X1")]
    pub samples: SampleCount,
    #[builder(default)]
    pub two_sided: bool,
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

#[derive(Debug, Default)]
pub struct VertexInputState {
    pub vertex_binding_descriptions: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attribute_descriptions: Vec<vk::VertexInputAttributeDescription>,
}
