use {
    super::{
        DescriptorBindingMap, Device, DriverError, PipelineDescriptorInfo, SampleCount, Shader,
        SpecializationInfo,
    },
    crate::{graph::AttachmentIndex, ptr::Shared},
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    ordered_float::OrderedFloat,
    std::{cmp::Ordering, collections::HashSet, ffi::CString, thread::panicking},
};

// TODO: Finally make this into a full struct and offer full features....
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlendMode {
    Alpha,
    PreMultipliedAlpha,
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
            Self::PreMultipliedAlpha => vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::TRUE,
                src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ONE,
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
    pub min: OrderedFloat<f32>,
    pub max: OrderedFloat<f32>,
    pub stencil_test: bool,
}

impl DepthStencilMode {
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
pub struct GraphicPipeline<P>
where
    P: SharedPointerKind,
{
    pub descriptor_bindings: DescriptorBindingMap,
    pub descriptor_info: PipelineDescriptorInfo<P>,
    device: Shared<Device<P>, P>,
    pub info: GraphicPipelineInfo,
    pub input_attachments: HashSet<AttachmentIndex>,
    pub layout: vk::PipelineLayout,
    pub push_constants: Vec<vk::PushConstantRange>,
    shader_modules: Vec<vk::ShaderModule>,
    stage_flags: vk::ShaderStageFlags,
    pub state: GraphicPipelineState,
    pub write_attachments: HashSet<AttachmentIndex>,
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

        let descriptor_bindings = shaders
            .iter()
            .map(|shader| shader.descriptor_bindings(&device))
            .collect::<Vec<_>>();
        let descriptor_bindings = Shader::merge_descriptor_bindings(descriptor_bindings);
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

            // Convert overlapping push constant regions such as this:
            // VERTEX 0..64
            // FRAGMENT 0..80
            //
            // To this:
            // VERTEX | FRAGMENT 0..64
            // FRAGMENT 64..80
            //
            // We do this now so that submission doesn't need to check for overlaps
            // See https://github.com/KhronosGroup/Vulkan-Docs/issues/609
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

#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Clone, Debug),
    pattern = "owned"
)]
pub struct GraphicPipelineInfo {
    #[builder(default)]
    pub blend: BlendMode,

    #[builder(default = "vk::CullModeFlags::BACK")]
    pub cull_mode: vk::CullModeFlags,

    #[builder(default, setter(strip_option))]
    pub depth_stencil: Option<DepthStencilMode>,

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
        Self {
            blend: BlendMode::default(),
            cull_mode: vk::CullModeFlags::BACK,
            depth_stencil: None,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            name: None,
            polygon_mode: vk::PolygonMode::FILL,
            samples: SampleCount::X1,
            two_sided: false,
        }
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
