// TODO: This file is way too repetitive with similar code blocks all over the place. It could use some lovin'.

use {
    super::spirv::{
        blending::{
            NORMAL_FRAG as BLEND_NORMAL_FRAG, QUAD_TRANSFORM_VERT as BLEND_QUAD_TRANSFORM_VERT,
        },
        deferred::{
            MESH_DUAL_FRAG, MESH_DUAL_VERT, MESH_SINGLE_FRAG, MESH_SINGLE_VERT, SPOTLIGHT_FRAG,
            SUNLIGHT_FRAG, TRANS_FRAG,
        },
        FONT_FRAG, FONT_OUTLINE_FRAG, FONT_VERT, GRADIENT_FRAG, GRADIENT_VERT, LINE_FRAG,
        LINE_VERT, QUAD_TRANSFORM_VERT, TEXTURE_FRAG,
    },
    crate::{
        color::TRANSPARENT_BLACK,
        gpu::driver::{
            descriptor_range_desc, descriptor_set_layout_binding, DescriptorPool,
            DescriptorSetLayout, Driver, GraphicsPipeline, PipelineLayout, Sampler, ShaderModule,
        },
    },
    gfx_hal::{
        format::Format,
        image::{Filter, Lod, WrapMode},
        pass::Subpass,
        pso::{
            AttributeDesc, BlendState, ColorBlendDesc, ColorMask, DescriptorPool as _,
            DescriptorRangeDesc, DescriptorSetLayoutBinding, DescriptorType, Element, EntryPoint,
            Face, FrontFace, GraphicsPipelineDesc, GraphicsShaderSet, ImageDescriptorType, LogicOp,
            PolygonMode, Primitive, Rasterizer, ShaderStageFlags, State, VertexBufferDesc,
            VertexInputRate,
        },
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

const FILL_RASTERIZER: Rasterizer = Rasterizer {
    conservative: false,
    cull_face: Face::NONE, // TODO: Face::BACK,
    depth_bias: None,
    depth_clamping: false,
    front_face: FrontFace::Clockwise,
    line_width: State::Static(1f32), // TODO: 0
    polygon_mode: PolygonMode::Fill,
};
const LINE_RASTERIZER: Rasterizer = Rasterizer {
    conservative: false,
    cull_face: Face::NONE,
    depth_bias: None,
    depth_clamping: false,
    front_face: FrontFace::Clockwise,
    line_width: State::Dynamic,
    polygon_mode: PolygonMode::Line,
};

fn sampler(driver: Driver, filter: Filter) -> Sampler {
    Sampler::new(
        driver,
        filter,
        filter,
        filter,
        (WrapMode::Tile, WrapMode::Tile, WrapMode::Tile),
        (Lod(0.0), Lod(0.0)..Lod(0.0)),
        None,
        TRANSPARENT_BLACK.into(),
        true,
        None,
    )
}

fn shader_set<'a>(
    vertex: EntryPoint<'a, _Backend>,
    fragment: EntryPoint<'a, _Backend>,
) -> GraphicsShaderSet<'a, _Backend> {
    GraphicsShaderSet {
        domain: None,
        fragment: Some(fragment),
        geometry: None,
        hull: None,
        vertex,
    }
}

#[derive(Clone, Copy, Default)]
pub struct FontVertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
}

#[derive(Debug)]
pub struct Graphics {
    desc_pool: DescriptorPool,
    desc_sets: Vec<<_Backend as Backend>::DescriptorSet>,
    layout: PipelineLayout,
    max_sets: usize,
    pipeline: GraphicsPipeline,
    samplers: Vec<Sampler>,
    set_layout: DescriptorSetLayout,
}

// TODO: VAST, VAST!!!, amounts of refactoring to be done here.
impl Graphics {
    /// # Safety
    /// None
    pub unsafe fn blend_normal(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &BLEND_QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &BLEND_NORMAL_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &[
                descriptor_set_layout_binding(
                    0,
                    1,
                    ShaderStageFlags::FRAGMENT,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Sampled { with_sampler: true },
                    },
                ),
                descriptor_set_layout_binding(
                    1,
                    1,
                    ShaderStageFlags::FRAGMENT,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Sampled { with_sampler: true },
                    },
                ),
            ],
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..72),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Copy);
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                2,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_line(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &LINE_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &LINE_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            empty::<DescriptorSetLayoutBinding>(),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::LineList,
            LINE_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        for _ in 0..4 {
            desc.blender.targets.push(ColorBlendDesc {
                blend: None,
                mask: ColorMask::empty(),
            });
        }
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 32,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rgba32Sfloat,
                offset: 12,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            empty::<DescriptorRangeDesc>(),
        );

        Self {
            desc_pool,
            desc_sets: vec![],
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_mesh_dual(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &MESH_DUAL_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &MESH_DUAL_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..100),
                (ShaderStageFlags::FRAGMENT, 100..104),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        for _ in 0..4 {
            desc.blender.targets.push(ColorBlendDesc {
                blend: None,
                mask: ColorMask::empty(),
            });
        }
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 32,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 12,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 2,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 24,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_mesh_single(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &MESH_SINGLE_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &MESH_SINGLE_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..100),
                (ShaderStageFlags::FRAGMENT, 100..104),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        for _ in 0..4 {
            desc.blender.targets.push(ColorBlendDesc {
                blend: None,
                mask: ColorMask::empty(),
            });
        }
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 32,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 12,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 2,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 24,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_spotlight(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &SPOTLIGHT_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_sunlight(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &SUNLIGHT_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &[descriptor_set_layout_binding(
                0,
                5,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )],
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        desc.blender.targets.push(ColorBlendDesc {
            blend: None,
            mask: ColorMask::empty(),
        });
        // desc.depth_stencil = DepthStencilDesc {
        //     depth: Some(DepthTest::PASS_WRITE),
        //     depth_bounds: true,
        //     stencil: Some(StencilTest {
        //         faces: Sided {
        //             back: StencilFace {
        //                 fun: Comparison::Never,
        //                 op_fail: StencilOp::Keep,
        //                 op_depth_fail: StencilOp::Keep,
        //                 op_pass: StencilOp::Keep,
        //             },
        //             front: StencilFace {
        //                 fun: Comparison::LessEqual,
        //                 op_fail: StencilOp::Zero,
        //                 op_depth_fail: StencilOp::Zero,
        //                 op_pass: StencilOp::Keep,
        //             },
        //         },
        //         read_masks: State::Static(Sided { back: 0, front: 0 }),
        //         write_masks: State::Static(Sided { back: 0, front: 0 }),
        //         reference_values: State::Static(Sided { back: 0, front: 0 }),
        //     }),
        // };
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                5,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: (0..=4)
                .map(|_| sampler(Driver::clone(&driver), Filter::Nearest))
                .collect(),
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_trans(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &MESH_SINGLE_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &TRANS_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &[
                descriptor_set_layout_binding(
                    0,
                    2,
                    ShaderStageFlags::FRAGMENT,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Sampled { with_sampler: true },
                    },
                ),
                descriptor_set_layout_binding(
                    1,
                    1,
                    ShaderStageFlags::FRAGMENT,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Sampled { with_sampler: true },
                    },
                ),
            ],
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..100),
                (ShaderStageFlags::FRAGMENT, 100..104),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 32,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rgb32Sfloat,
                offset: 12,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 2,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 24,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                3,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: (0..3)
                .map(|_| sampler(Driver::clone(&driver), Filter::Nearest))
                .collect(),
        }
    }

    pub unsafe fn font(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &FONT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &FONT_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..80),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 16,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 8,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn font_outline(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &FONT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &FONT_OUTLINE_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..96),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: 16,
            rate: VertexInputRate::Vertex,
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 0,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 0,
            },
        });
        desc.attributes.push(AttributeDesc {
            binding: 0,
            location: 1,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 8,
            },
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn gradient(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &GRADIENT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &GRADIENT_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::FRAGMENT, 0..32)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn gradient_transparency(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &GRADIENT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &GRADIENT_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::FRAGMENT, 0..32)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn texture(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &TEXTURE_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(descriptor_set_layout_binding(
                0,
                1,
                ShaderStageFlags::FRAGMENT,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let layout = PipelineLayout::new(
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            shader_set(
                ShaderModule::entry_point(&vertex),
                ShaderModule::entry_point(&fragment),
            ),
            Primitive::TriangleList,
            FILL_RASTERIZER,
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::PREMULTIPLIED_ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );
        let mut desc_pool = DescriptorPool::new(
            Driver::clone(&driver),
            max_sets,
            once(descriptor_range_desc(
                1,
                DescriptorType::Image {
                    ty: ImageDescriptorType::Sampled { with_sampler: true },
                },
            )),
        );
        let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        Self {
            desc_pool,
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout,
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub fn desc_set(&self, idx: usize) -> &<_Backend as Backend>::DescriptorSet {
        &self.desc_sets[idx]
    }

    pub fn layout(&self) -> &PipelineLayout {
        &self.layout
    }

    pub fn max_sets(&self) -> usize {
        self.max_sets
    }

    pub fn pipeline(&self) -> &GraphicsPipeline {
        &self.pipeline
    }

    fn reset(&mut self) {
        unsafe {
            self.desc_pool.reset();
        }

        for desc_set in &mut self.desc_sets {
            *desc_set = unsafe { self.desc_pool.allocate_set(&*self.set_layout).unwrap() }
        }
    }

    pub fn sampler(&self, idx: usize) -> &Sampler {
        &self.samplers[idx]
    }
}
