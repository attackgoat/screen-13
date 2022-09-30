use {
    super::{
        merge_push_constant_ranges, DescriptorBindingMap, Device, DriverError,
        PipelineDescriptorInfo, SampleCount, Shader, SpecializationInfo,
    },
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    ordered_float::OrderedFloat,
    std::{collections::HashSet, ffi::CString, sync::Arc, thread::panicking},
};

const RGBA_COLOR_COMPONENTS: vk::ColorComponentFlags = vk::ColorComponentFlags::from_raw(
    vk::ColorComponentFlags::R.as_raw()
        | vk::ColorComponentFlags::G.as_raw()
        | vk::ColorComponentFlags::B.as_raw()
        | vk::ColorComponentFlags::A.as_raw(),
);

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Debug),
    pattern = "owned"
)]
pub struct BlendMode {
    #[builder(default = "false")]
    pub blend_enable: bool,
    #[builder(default = "vk::BlendFactor::SRC_COLOR")]
    pub src_color_blend_factor: vk::BlendFactor,
    #[builder(default = "vk::BlendFactor::ONE_MINUS_DST_COLOR")]
    pub dst_color_blend_factor: vk::BlendFactor,
    #[builder(default = "vk::BlendOp::ADD")]
    pub color_blend_op: vk::BlendOp,
    #[builder(default = "vk::BlendFactor::ZERO")]
    pub src_alpha_blend_factor: vk::BlendFactor,
    #[builder(default = "vk::BlendFactor::ZERO")]
    pub dst_alpha_blend_factor: vk::BlendFactor,
    #[builder(default = "vk::BlendOp::ADD")]
    pub alpha_blend_op: vk::BlendOp,
    #[builder(default = "RGBA_COLOR_COMPONENTS")]
    pub color_write_mask: vk::ColorComponentFlags,
}

impl BlendModeBuilder {
    pub fn build(self) -> BlendMode {
        self.fallible_build().unwrap()
    }
}

impl BlendMode {
    pub const REPLACE: Self = Self {
        blend_enable: false,
        src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ZERO,
        dst_alpha_blend_factor: vk::BlendFactor::ZERO,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: RGBA_COLOR_COMPONENTS,
    };
    pub const ALPHA: Self = Self {
        blend_enable: true,
        src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::SRC_ALPHA,
        dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: RGBA_COLOR_COMPONENTS,
    };
    pub const PRE_MULTIPLIED_ALPHA: Self = Self {
        blend_enable: true,
        src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ONE,
        dst_alpha_blend_factor: vk::BlendFactor::ONE,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: RGBA_COLOR_COMPONENTS,
    };

    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> BlendModeBuilder {
        BlendModeBuilder::default()
    }

    pub fn into_vk(&self) -> vk::PipelineColorBlendAttachmentState {
        vk::PipelineColorBlendAttachmentState {
            blend_enable: if self.blend_enable {
                vk::TRUE
            } else {
                vk::FALSE
            },
            src_color_blend_factor: self.src_color_blend_factor,
            dst_color_blend_factor: self.dst_color_blend_factor,
            color_blend_op: self.color_blend_op,
            src_alpha_blend_factor: self.src_alpha_blend_factor,
            dst_alpha_blend_factor: self.dst_alpha_blend_factor,
            alpha_blend_op: self.alpha_blend_op,
            color_write_mask: self.color_write_mask,
        }
    }
}

// the Builder derive Macro wants Default to be implemented for BlendMode
impl Default for BlendMode {
    fn default() -> Self {
        Self::REPLACE
    }
}

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Debug),
    pattern = "owned"
)]
pub struct DepthStencilMode {
    pub back: StencilMode,
    pub bounds_test: bool,
    pub compare_op: vk::CompareOp,
    pub depth_test: bool,
    pub depth_write: bool,
    pub front: StencilMode,

    // Note: Using setter(into) so caller does not need our version of OrderedFloat
    #[builder(setter(into))]
    pub min: OrderedFloat<f32>,
    #[builder(setter(into))]
    pub max: OrderedFloat<f32>,

    pub stencil_test: bool,
}

impl DepthStencilMode {
    pub const DEPTH_READ: Self = Self {
        back: StencilMode::IGNORE,
        bounds_test: true,
        compare_op: vk::CompareOp::LESS,
        depth_test: true,
        depth_write: false,
        front: StencilMode::IGNORE,
        min: OrderedFloat(0.0),
        max: OrderedFloat(1.0),
        stencil_test: false,
    };
    pub const DEPTH_WRITE: Self = Self {
        back: StencilMode::IGNORE,
        bounds_test: true,
        compare_op: vk::CompareOp::LESS,
        depth_test: true,
        depth_write: true,
        front: StencilMode::IGNORE,
        min: OrderedFloat(0.0),
        max: OrderedFloat(1.0),
        stencil_test: false,
    };

    pub const IGNORE: Self = Self {
        back: StencilMode::IGNORE,
        bounds_test: false,
        compare_op: vk::CompareOp::NEVER,
        depth_test: false,
        depth_write: false,
        front: StencilMode::IGNORE,
        min: OrderedFloat(0.0),
        max: OrderedFloat(0.0),
        stencil_test: false,
    };

    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> DepthStencilModeBuilder {
        DepthStencilModeBuilder::default()
    }

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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl DepthStencilModeBuilder {
    pub fn build(mut self) -> DepthStencilMode {
        if self.back.is_none() {
            self.back = Some(Default::default());
        }

        if self.bounds_test.is_none() {
            self.bounds_test = Some(Default::default());
        }

        if self.compare_op.is_none() {
            self.compare_op = Some(Default::default());
        }

        if self.depth_test.is_none() {
            self.depth_test = Some(Default::default());
        }

        if self.depth_write.is_none() {
            self.depth_write = Some(Default::default());
        }

        if self.front.is_none() {
            self.front = Some(Default::default());
        }

        if self.min.is_none() {
            self.min = Some(Default::default());
        }

        if self.max.is_none() {
            self.max = Some(Default::default());
        }

        if self.stencil_test.is_none() {
            self.stencil_test = Some(Default::default());
        }

        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
pub struct GraphicPipeline {
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,
    pub info: GraphicPipelineInfo,
    pub input_attachments: HashSet<u32>,
    pub layout: vk::PipelineLayout,
    pub push_constants: Vec<vk::PushConstantRange>,
    shader_modules: Vec<vk::ShaderModule>,
    stage_flags: vk::ShaderStageFlags,
    pub state: GraphicPipelineState,
    pub write_attachments: HashSet<u32>,
}

impl GraphicPipeline {
    pub fn create<S>(
        device: &Arc<Device>,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Result<Self, DriverError>
    where
        S: Into<Shader>,
    {
        trace!("create");

        let device = Arc::clone(device);
        let info = info.into();
        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Vec<Shader>>();

        let vertex_input = shaders
            .iter()
            .find(|shader| shader.stage == vk::ShaderStageFlags::VERTEX)
            .expect("vertex shader not found")
            .vertex_input();

        // Check for proper stages because vulkan may not complain but this is bad
        let has_fragment_stage = shaders
            .iter()
            .any(|shader| shader.stage.contains(vk::ShaderStageFlags::FRAGMENT));
        let has_tesselation_stage = shaders.iter().any(|shader| {
            shader
                .stage
                .contains(vk::ShaderStageFlags::TESSELLATION_CONTROL)
        }) && shaders.iter().any(|shader| {
            shader
                .stage
                .contains(vk::ShaderStageFlags::TESSELLATION_EVALUATION)
        });
        let has_geometry_stage = shaders
            .iter()
            .any(|shader| shader.stage.contains(vk::ShaderStageFlags::GEOMETRY));

        debug_assert!(
            has_fragment_stage || has_tesselation_stage || has_geometry_stage,
            "invalid shader stage combination"
        );

        let mut descriptor_bindings = Shader::merge_descriptor_bindings(
            shaders
                .iter()
                .map(|shader| shader.descriptor_bindings(&device)),
        );
        for (descriptor_info, _) in descriptor_bindings.values_mut() {
            if descriptor_info.binding_count() == 0 {
                descriptor_info.set_binding_count(info.bindless_descriptor_count);
            }
        }

        let descriptor_info = PipelineDescriptorInfo::create(&device, &descriptor_bindings)?;
        let descriptor_sets_layouts = descriptor_info
            .layouts
            .iter()
            .map(|(_, descriptor_set_layout)| **descriptor_set_layout)
            .collect::<Box<[_]>>();

        let mut push_constants = shaders
            .iter()
            .map(|shader| shader.push_constant_range())
            .filter_map(|mut push_const| push_const.take())
            .collect::<Vec<_>>();

        let (input_attachments, write_attachments) = {
            let (input, write) = shaders
                .iter()
                .find(|shader| shader.stage == vk::ShaderStageFlags::FRAGMENT)
                .expect("fragment shader not found")
                .attachments();
            let (input, write) = (input.collect(), write.collect());

            for input in &input {
                trace!("detected input attachment {input}");
            }

            for write in &write {
                trace!("detected write attachment {write}");
            }

            (input, write)
        };

        unsafe {
            let layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .set_layouts(&descriptor_sets_layouts)
                        .push_constant_ranges(&push_constants),
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

            let stage_flags = stages
                .iter()
                .map(|stage| stage.flags)
                .reduce(|j, k| j | k)
                .unwrap_or_default();

            merge_push_constant_ranges(&mut push_constants);

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                input_attachments,
                layout,
                push_constants,
                shader_modules,
                stage_flags,
                state: GraphicPipelineState {
                    layout,
                    multisample,
                    rasterization,
                    stages,
                    vertex_input,
                },
                write_attachments,
            })
        }
    }

    pub fn stages(&self) -> vk::ShaderStageFlags {
        self.stage_flags
    }
}

impl Drop for GraphicPipeline {
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

#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Clone, Debug),
    pattern = "owned"
)]
pub struct GraphicPipelineInfo {
    #[builder(default)]
    pub blend: BlendMode,

    #[builder(default = "8192")]
    pub bindless_descriptor_count: u32,

    #[builder(default = "vk::CullModeFlags::BACK")]
    pub cull_mode: vk::CullModeFlags,

    #[builder(default = "vk::FrontFace::COUNTER_CLOCKWISE")]
    pub front_face: vk::FrontFace,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,

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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl GraphicPipelineInfoBuilder {
    pub fn build(self) -> GraphicPipelineInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

impl Default for GraphicPipelineInfo {
    fn default() -> Self {
        Self::new().build()
    }
}

impl From<GraphicPipelineInfoBuilder> for GraphicPipelineInfo {
    fn from(info: GraphicPipelineInfoBuilder) -> Self {
        info.build()
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StencilMode {
    pub fail_op: vk::StencilOp,
    pub pass_op: vk::StencilOp,
    pub depth_fail_op: vk::StencilOp,
    pub compare_op: vk::CompareOp,
    pub compare_mask: u32,
    pub write_mask: u32,
    pub reference: u32,
}

impl StencilMode {
    pub const IGNORE: Self = Self {
        fail_op: vk::StencilOp::KEEP,
        pass_op: vk::StencilOp::KEEP,
        depth_fail_op: vk::StencilOp::KEEP,
        compare_op: vk::CompareOp::NEVER,
        compare_mask: 0,
        write_mask: 0,
        reference: 0,
    };

    fn into_vk(self) -> vk::StencilOpState {
        vk::StencilOpState {
            fail_op: self.fail_op,
            pass_op: self.pass_op,
            depth_fail_op: self.depth_fail_op,
            compare_op: self.compare_op,
            compare_mask: self.compare_mask,
            write_mask: self.write_mask,
            reference: self.reference,
        }
    }
}

impl Default for StencilMode {
    fn default() -> Self {
        Self::IGNORE
    }
}

#[derive(Debug, Default)]
pub struct VertexInputState {
    pub vertex_binding_descriptions: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attribute_descriptions: Vec<vk::VertexInputAttributeDescription>,
}
