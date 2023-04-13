//! Graphics pipeline types

use {
    super::{
        image::SampleCount,
        merge_push_constant_ranges,
        shader::{DescriptorBindingMap, PipelineDescriptorInfo, Shader, SpecializationInfo},
        Device, DriverError,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
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

/// Specifies color blend state used when rasterization is enabled for any color attachments
/// accessed during rendering.
///
/// See
/// [VkPipelineColorBlendAttachmentState](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPipelineColorBlendAttachmentState.html).
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build", error = "BlendModeBuilderError"),
    derive(Debug),
    pattern = "owned"
)]
pub struct BlendMode {
    /// Controls whether blending is enabled for the corresponding color attachment.
    ///
    /// If blending is not enabled, the source fragment’s color for that attachment is passed
    /// through unmodified.
    #[builder(default = "false")]
    pub blend_enable: bool,

    /// Selects which blend factor is used to determine the source factors.
    #[builder(default = "vk::BlendFactor::SRC_COLOR")]
    pub src_color_blend_factor: vk::BlendFactor,

    /// Selects which blend factor is used to determine the destination factors.
    #[builder(default = "vk::BlendFactor::ONE_MINUS_DST_COLOR")]
    pub dst_color_blend_factor: vk::BlendFactor,

    /// Selects which blend operation is used to calculate the RGB values to write to the color
    /// attachment.
    #[builder(default = "vk::BlendOp::ADD")]
    pub color_blend_op: vk::BlendOp,

    /// Selects which blend factor is used to determine the source factor.
    #[builder(default = "vk::BlendFactor::ZERO")]
    pub src_alpha_blend_factor: vk::BlendFactor,

    /// Selects which blend factor is used to determine the destination factor.
    #[builder(default = "vk::BlendFactor::ZERO")]
    pub dst_alpha_blend_factor: vk::BlendFactor,

    /// Selects which blend operation is used to calculate the alpha values to write to the color
    /// attachment.
    #[builder(default = "vk::BlendOp::ADD")]
    pub alpha_blend_op: vk::BlendOp,

    /// A bitmask of specifying which of the R, G, B, and/or A components are enabled for writing,
    /// as described for the
    /// [Color Write Mask](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#framebuffer-color-write-mask).
    #[builder(default = "RGBA_COLOR_COMPONENTS")]
    pub color_write_mask: vk::ColorComponentFlags,
}

impl BlendMode {
    /// A commonly used blend mode for replacing color attachment values with new ones.
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

    /// A commonly used blend mode for blending color attachment values based on the alpha channel.
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

    /// A commonly used blend mode for blending color attachment values based on the alpha channel,
    /// where the color components have been pre-multiplied with the alpha component value.
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

    /// Specifies a default blend mode which is not enabled.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> BlendModeBuilder {
        BlendModeBuilder::default()
    }

    pub(super) fn into_vk(self) -> vk::PipelineColorBlendAttachmentState {
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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl BlendModeBuilder {
    /// Builds a new `BlendMode`.
    pub fn build(self) -> BlendMode {
        self.fallible_build().unwrap()
    }
}

#[derive(Debug)]
struct BlendModeBuilderError;

impl From<UninitializedFieldError> for BlendModeBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Specifies the [depth bounds tests], [stencil test], and [depth test] pipeline state.
///
/// [depth bounds tests]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-dbt
/// [stencil test]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-stencil
/// [depth test]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-depth
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[builder(
    build_fn(
        private,
        name = "fallible_build",
        error = "DepthStencilModeBuilderError"
    ),
    derive(Debug),
    pattern = "owned"
)]
pub struct DepthStencilMode {
    /// Control parameters of the stencil test.
    pub back: StencilMode,

    /// Controls whether [depth bounds testing] is enabled.
    ///
    /// [depth bounds testing]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-dbt
    pub bounds_test: bool,

    /// A value specifying the comparison operator to use in the [depth comparison] step of the
    /// [depth test].
    ///
    /// [depth comparison]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-depth-comparison
    /// [depth test]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-depth
    pub compare_op: vk::CompareOp,

    /// Controls whether [depth testing] is enabled.
    ///
    /// [depth testing]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-depth
    pub depth_test: bool,

    /// Controls whether [depth writes] are enabled when `depth_test` is `true`.
    ///
    /// Depth writes are always disabled when `depth_test` is `false`.
    ///
    /// [depth writes]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-depth-write
    pub depth_write: bool,

    /// Control parameters of the stencil test.
    pub front: StencilMode,

    // Note: Using setter(into) so caller does not need our version of OrderedFloat
    /// Minimum depth bound used in the [depth bounds test].
    ///
    /// [depth bounds test]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-dbt
    #[builder(setter(into))]
    pub min: OrderedFloat<f32>,

    // Note: Using setter(into) so caller does not need our version of OrderedFloat
    /// Maximum depth bound used in the [depth bounds test].
    ///
    /// [depth bounds test]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-dbt
    #[builder(setter(into))]
    pub max: OrderedFloat<f32>,

    /// Controls whether [stencil testing] is enabled.
    ///
    /// [stencil testing]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-stencil
    pub stencil_test: bool,
}

impl DepthStencilMode {
    /// A commonly used depth/stencil mode
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

    /// A commonly used depth/stencil mode
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

    /// Specifies a no-depth/no-stencil mode.
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

    /// Specifies a default depth/stencil mode which is equal to [`DepthStencilMode::IGNORE`].
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
    /// Builds a new `DepthStencilMode`.
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
struct DepthStencilModeBuilderError;

impl From<UninitializedFieldError> for DepthStencilModeBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Opaque representation of a [pipeline] object.
///
/// Also contains information about the object.
///
/// [pipeline]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPipeline.html
#[derive(Debug)]
pub struct GraphicPipeline {
    pub(crate) descriptor_bindings: DescriptorBindingMap,
    pub(crate) descriptor_info: PipelineDescriptorInfo,
    device: Arc<Device>,

    /// Information used to create this object.
    pub info: GraphicPipelineInfo,

    pub(crate) input_attachments: HashSet<u32>,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) push_constants: Vec<vk::PushConstantRange>,
    shader_modules: Vec<vk::ShaderModule>,
    pub(super) state: GraphicPipelineState,

    // Used in debug mode
    #[allow(dead_code)]
    pub(crate) write_attachments: HashSet<u32>,
}

impl GraphicPipeline {
    /// Creates a new graphic pipeline on the given device.
    ///
    /// The correct pipeline stages will be enabled based on the provided shaders. See [Shader] for
    /// details on all available stages.
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
    /// # use screen_13::driver::graphic::{GraphicPipeline, GraphicPipelineInfo};
    /// # use screen_13::driver::shader::Shader;
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let my_frag_code = [0u8; 1];
    /// # let my_vert_code = [0u8; 1];
    /// // shader code is raw SPIR-V code as bytes
    /// let vert = Shader::new_vertex(my_vert_code.as_slice());
    /// let frag = Shader::new_fragment(my_frag_code.as_slice());
    /// let info = GraphicPipelineInfo::default();
    /// let pipeline = GraphicPipeline::create(&device, info, [vert, frag])?;
    ///
    /// assert_eq!(pipeline.info.front_face, vk::FrontFace::COUNTER_CLOCKWISE);
    /// # Ok(()) }
    /// ```
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
            .values()
            .map(|descriptor_set_layout| **descriptor_set_layout)
            .collect::<Box<[_]>>();

        let push_constants = shaders
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

            let multisample = MultisampleState {
                rasterization_samples: info.samples,
                ..Default::default()
            };

            let push_constants = merge_push_constant_ranges(&push_constants);

            Ok(Self {
                descriptor_bindings,
                descriptor_info,
                device,
                info,
                input_attachments,
                layout,
                push_constants,
                shader_modules,
                state: GraphicPipelineState {
                    layout,
                    multisample,
                    stages,
                    vertex_input,
                },
                write_attachments,
            })
        }
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

/// Information used to create a [`GraphicPipeline`] instance.
#[derive(Builder, Clone, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(
        private,
        name = "fallible_build",
        error = "GraphicPipelineInfoBuilderError"
    ),
    derive(Clone, Debug),
    pattern = "owned"
)]
pub struct GraphicPipelineInfo {
    /// Specifies color blend state used when rasterization is enabled for any color attachments
    /// accessed during rendering.
    ///
    /// The default value is [`BlendMode::REPLACE`].
    #[builder(default)]
    pub blend: BlendMode,

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
    /// layout(set = 0, binding = 0) uniform sampler2D my_binding[];
    ///
    /// void main()
    /// {
    ///     // my_binding will have space for 8,192 images by default
    /// }
    /// # "#, frag);
    /// ```
    #[builder(default = "8192")]
    pub bindless_descriptor_count: u32,

    /// Bitmask controlling triangle culling.
    ///
    /// The default value is `vk::CullModeFlags::BACK`.
    #[builder(default = "vk::CullModeFlags::BACK")]
    pub cull_mode: vk::CullModeFlags,

    /// Interpret polygon front-facing orientation.
    ///
    /// The default value is `vk::FrontFace::COUNTER_CLOCKWISE`.
    #[builder(default = "vk::FrontFace::COUNTER_CLOCKWISE")]
    pub front_face: vk::FrontFace,

    /// A descriptive name used in debugging messages.
    #[builder(default, setter(strip_option))]
    pub name: Option<String>,

    /// Control polygon rasterization mode.
    ///
    /// The default value is `vk::PolygonMode::FILL`.
    #[builder(default = "vk::PolygonMode::FILL")]
    pub polygon_mode: vk::PolygonMode,

    /// Input primitive topology.
    ///
    /// The default value is `vk::PrimitiveTopology::TRIANGLE_LIST`.
    #[builder(default = "vk::PrimitiveTopology::TRIANGLE_LIST")]
    pub topology: vk::PrimitiveTopology,

    /// Multisampling antialias mode.
    ///
    /// The default value is `SampleCount::X1`.
    ///
    /// See [multisampling](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#primsrast-multisampling).
    #[builder(default = "SampleCount::X1")]
    pub samples: SampleCount,
}

impl GraphicPipelineInfo {
    /// Specifies a graphic pipeline.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> GraphicPipelineInfoBuilder {
        GraphicPipelineInfoBuilder::default()
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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl GraphicPipelineInfoBuilder {
    /// Builds a new `GraphicPipelineInfo`.
    pub fn build(self) -> GraphicPipelineInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
struct GraphicPipelineInfoBuilderError;

impl From<UninitializedFieldError> for GraphicPipelineInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

#[derive(Debug)]
pub(super) struct GraphicPipelineState {
    pub layout: vk::PipelineLayout,
    pub multisample: MultisampleState,
    pub stages: Vec<Stage>,
    pub vertex_input: VertexInputState,
}

#[derive(Debug, Default)]
pub(super) struct MultisampleState {
    pub alpha_to_coverage_enable: bool,
    pub alpha_to_one_enable: bool,
    pub flags: vk::PipelineMultisampleStateCreateFlags,
    pub min_sample_shading: f32,
    pub rasterization_samples: SampleCount,
    pub sample_mask: Vec<u32>,
    pub sample_shading_enable: bool,
}

#[derive(Debug)]
pub(super) struct Stage {
    pub flags: vk::ShaderStageFlags,
    pub module: vk::ShaderModule,
    pub name: CString,
    pub specialization_info: Option<SpecializationInfo>,
}

/// Specifies stencil mode during rasterization.
///
/// See
/// [stencil test](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fragops-stencil).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StencilMode {
    /// The action performed on samples that fail the stencil test.
    pub fail_op: vk::StencilOp,

    /// The action performed on samples that pass both the depth and stencil tests.
    pub pass_op: vk::StencilOp,

    /// The action performed on samples that pass the stencil test and fail the depth test.
    pub depth_fail_op: vk::StencilOp,

    /// The comparison operator used in the stencil test.
    pub compare_op: vk::CompareOp,

    /// The bits of the unsigned integer stencil values participating in the stencil test.
    pub compare_mask: u32,

    /// The bits of the unsigned integer stencil values updated by the stencil test in the stencil
    /// framebuffer attachment.
    pub write_mask: u32,

    /// An unsigned integer stencil reference value that is used in the unsigned stencil comparison.
    pub reference: u32,
}

impl StencilMode {
    /// Specifes a stencil mode which is has no effect.
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

#[derive(Clone, Debug, Default)]
pub(super) struct VertexInputState {
    pub vertex_binding_descriptions: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attribute_descriptions: Vec<vk::VertexInputAttributeDescription>,
}
