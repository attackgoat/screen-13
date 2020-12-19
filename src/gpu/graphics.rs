// TODO: This file is way too repetitive with similar code blocks all over the place. It could use some lovin'.

use {
    super::spirv,
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
            AttributeDesc, BlendState, ColorBlendDesc, ColorMask, Comparison, DepthTest,
            DescriptorPool as _, DescriptorRangeDesc, DescriptorSetLayoutBinding, DescriptorType,
            Element, Face, FrontFace, GraphicsPipelineDesc, ImageDescriptorType,
            InputAssemblerDesc, LogicOp, PolygonMode, Primitive, PrimitiveAssemblerDesc,
            Rasterizer, ShaderStageFlags, State, VertexBufferDesc, VertexInputRate,
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
    line_width: State::Static(1.0),
    polygon_mode: PolygonMode::Fill,
};
const LINE_RASTERIZER: Rasterizer = Rasterizer {
    conservative: false,
    cull_face: Face::NONE,
    depth_bias: None,
    depth_clamping: false,
    front_face: FrontFace::Clockwise,
    line_width: State::Static(1.0),
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

pub struct Graphics {
    desc_pool: Option<DescriptorPool>,
    desc_sets: Vec<<_Backend as Backend>::DescriptorSet>,
    layout: PipelineLayout,
    max_sets: usize,
    pipeline: GraphicsPipeline,
    samplers: Vec<Sampler>,
    set_layout: Option<DescriptorSetLayout>,
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
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::blend::QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::blend::NORMAL_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..72),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
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
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::defer::LINE_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::defer::LINE_FRAG);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            empty::<DescriptorSetLayoutBinding>(),
        );
        let layout = PipelineLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[
                    AttributeDesc {
                        binding: 0,
                        location: 0,
                        element: Element {
                            format: Format::Rgb32Sfloat,
                            offset: 0,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 1,
                        element: Element {
                            format: Format::Rgba32Sfloat,
                            offset: 12,
                        },
                    },
                ],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 32,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::LineList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            LINE_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets: vec![],
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_mesh(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::defer::MESH_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::defer::MESH_FRAG);
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
                descriptor_set_layout_binding(
                    2,
                    1,
                    ShaderStageFlags::FRAGMENT,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Sampled { with_sampler: true },
                    },
                ),
            ],
        );
        let layout = PipelineLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[
                    AttributeDesc {
                        binding: 0,
                        location: 0,
                        element: Element {
                            format: Format::Rgb32Sfloat,
                            offset: 0,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 1,
                        element: Element {
                            format: Format::Rgb32Sfloat,
                            offset: 12,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 2,
                        element: Element {
                            format: Format::Rgba32Sfloat,
                            offset: 24,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 3,
                        element: Element {
                            format: Format::Rg32Sfloat,
                            offset: 56,
                        },
                    },
                ],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 64,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        for _ in 0..2 {
            desc.blender.targets.push(ColorBlendDesc {
                blend: None,
                mask: ColorMask::ALL,
            });
        }
        desc.depth_stencil.depth = Some(DepthTest {
            fun: Comparison::LessEqual,
            write: true,
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![
                sampler(Driver::clone(&driver), Filter::Nearest),
                sampler(Driver::clone(&driver), Filter::Nearest),
                sampler(Driver::clone(&driver), Filter::Nearest),
            ],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_point_light(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        _max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::defer::LIGHT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::defer::POINT_LIGHT_FRAG);
        let layout = PipelineLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            empty::<&<_Backend as Backend>::DescriptorSetLayout>(),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 0..0),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[AttributeDesc {
                    binding: 0,
                    location: 0,
                    element: Element {
                        format: Format::Rgb32Sfloat,
                        offset: 0,
                    },
                }],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 12,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::ADD),
            mask: ColorMask::RED,
        });
        desc.depth_stencil.depth = Some(DepthTest {
            fun: Comparison::LessEqual,
            write: false,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );

        Self {
            desc_pool: None,
            desc_sets: vec![],
            layout,
            max_sets: 0,
            pipeline,
            set_layout: None,
            samplers: vec![],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_rect_light(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        _max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::defer::LIGHT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::defer::POINT_LIGHT_FRAG);
        let layout = PipelineLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            empty::<&<_Backend as Backend>::DescriptorSetLayout>(),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 0..0),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[AttributeDesc {
                    binding: 0,
                    location: 0,
                    element: Element {
                        format: Format::Rgb32Sfloat,
                        offset: 0,
                    },
                }],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 12,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::ADD),
            mask: ColorMask::RED,
        });
        desc.depth_stencil.depth = Some(DepthTest {
            fun: Comparison::LessEqual,
            write: false,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            &desc,
        );

        Self {
            desc_pool: None,
            desc_sets: vec![],
            layout,
            max_sets: 0,
            pipeline,
            set_layout: None,
            samplers: vec![],
        }
    }

    /// # Safety
    /// None
    pub unsafe fn draw_spotlight(
        #[cfg(debug_assertions)] _name: &str,
        _driver: &Driver,
        _subpass: Subpass<'_, _Backend>,
        _max_sets: usize,
    ) -> Self {
        // let vertex = ShaderModule::new(Driver::clone(&driver), &QUAD_TRANSFORM_VERT);
        // let fragment = ShaderModule::new(Driver::clone(&driver), &SPOTLIGHT_FRAG);
        // let set_layout = DescriptorSetLayout::new(
        //     #[cfg(debug_assertions)]
        //     name,
        //     Driver::clone(&driver),
        //     once(descriptor_set_layout_binding(
        //         0,
        //         1,
        //         ShaderStageFlags::FRAGMENT,
        //         DescriptorType::Image {
        //             ty: ImageDescriptorType::Sampled { with_sampler: true },
        //         },
        //     )),
        // );
        // let layout = PipelineLayout::new(
        //     Driver::clone(&driver),
        //     once(&*set_layout),
        //     &[(ShaderStageFlags::VERTEX, 0..64)],
        // );
        // let mut desc = GraphicsPipelineDesc::new(
        //     PrimitiveAssemblerDesc::Vertex {
        //         attributes: &[],
        //         buffers: &[],
        //         geometry: None,
        //         input_assembler: InputAssemblerDesc {
        //             primitive: Primitive::TriangleList,
        //             restart_index: None,
        //             with_adjacency: false,
        //         },
        //         tessellation: None,
        //         vertex: ShaderModule::entry_point(&vertex),
        //     },
        //     FILL_RASTERIZER,
        //     Some(ShaderModule::entry_point(&fragment)),
        //     &layout,
        //     subpass,
        // );
        // desc.blender.logic_op = Some(LogicOp::Set);
        // desc.blender.targets.push(ColorBlendDesc {
        //     blend: Some(BlendState::PREMULTIPLIED_ALPHA),
        //     mask: ColorMask::ALL,
        // });
        // let pipeline = GraphicsPipeline::new(
        //     #[cfg(debug_assertions)]
        //     name,
        //     Driver::clone(&driver),
        //     &desc,
        // );
        // let mut desc_pool = DescriptorPool::new(
        //     Driver::clone(&driver),
        //     max_sets,
        //     once(descriptor_range_desc(
        //         1,
        //         DescriptorType::Image {
        //             ty: ImageDescriptorType::Sampled { with_sampler: true },
        //         },
        //     )),
        // );
        // let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        // Self {
        //     desc_pool,
        //     desc_sets,
        //     layout,
        //     max_sets,
        //     pipeline,
        //     set_layout,
        //     samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        // }
        todo!();
    }

    /// # Safety
    /// None
    pub unsafe fn draw_sunlight(
        #[cfg(debug_assertions)] _name: &str,
        _driver: &Driver,
        _subpass: Subpass<'_, _Backend>,
        _max_sets: usize,
    ) -> Self {
        // let vertex = ShaderModule::new(Driver::clone(&driver), &QUAD_TRANSFORM_VERT);
        // let fragment = ShaderModule::new(Driver::clone(&driver), &SUNLIGHT_FRAG);
        // let set_layout = DescriptorSetLayout::new(
        //     #[cfg(debug_assertions)]
        //     name,
        //     Driver::clone(&driver),
        //     &[descriptor_set_layout_binding(
        //         0,
        //         5,
        //         ShaderStageFlags::FRAGMENT,
        //         DescriptorType::Image {
        //             ty: ImageDescriptorType::Sampled { with_sampler: true },
        //         },
        //     )],
        // );
        // let layout = PipelineLayout::new(
        //     Driver::clone(&driver),
        //     once(&*set_layout),
        //     &[(ShaderStageFlags::VERTEX, 0..64)],
        // );
        // let mut desc = GraphicsPipelineDesc::new(
        //     PrimitiveAssemblerDesc::Vertex {
        //         attributes: &[],
        //         buffers: &[],
        //         geometry: None,
        //         input_assembler: InputAssemblerDesc {
        //             primitive: Primitive::TriangleList,
        //             restart_index: None,
        //             with_adjacency: false,
        //         },
        //         tessellation: None,
        //         vertex: ShaderModule::entry_point(&vertex),
        //     },
        //     FILL_RASTERIZER,
        //     Some(ShaderModule::entry_point(&fragment)),
        //     &layout,
        //     subpass,
        // );
        // desc.blender.logic_op = Some(LogicOp::Set);
        // desc.blender.targets.push(ColorBlendDesc {
        //     blend: None,
        //     mask: ColorMask::empty(),
        // });
        // // desc.depth_stencil = DepthStencilDesc {
        // //     depth: Some(DepthTest::PASS_WRITE),
        // //     depth_bounds: true,
        // //     stencil: Some(StencilTest {
        // //         faces: Sided {
        // //             back: StencilFace {
        // //                 fun: Comparison::Never,
        // //                 op_fail: StencilOp::Keep,
        // //                 op_depth_fail: StencilOp::Keep,
        // //                 op_pass: StencilOp::Keep,
        // //             },
        // //             front: StencilFace {
        // //                 fun: Comparison::LessEqual,
        // //                 op_fail: StencilOp::Zero,
        // //                 op_depth_fail: StencilOp::Zero,
        // //                 op_pass: StencilOp::Keep,
        // //             },
        // //         },
        // //         read_masks: State::Static(Sided { back: 0, front: 0 }),
        // //         write_masks: State::Static(Sided { back: 0, front: 0 }),
        // //         reference_values: State::Static(Sided { back: 0, front: 0 }),
        // //     }),
        // // };
        // let pipeline = GraphicsPipeline::new(
        //     #[cfg(debug_assertions)]
        //     name,
        //     Driver::clone(&driver),
        //     &desc,
        // );
        // let mut desc_pool = DescriptorPool::new(
        //     Driver::clone(&driver),
        //     max_sets,
        //     once(descriptor_range_desc(
        //         5,
        //         DescriptorType::Image {
        //             ty: ImageDescriptorType::Sampled { with_sampler: true },
        //         },
        //     )),
        // );
        // let desc_sets = vec![desc_pool.allocate_set(&*set_layout).unwrap()];

        // Self {
        //     desc_pool,
        //     desc_sets,
        //     layout,
        //     max_sets,
        //     pipeline,
        //     set_layout,
        //     samplers: (0..=4)
        //         .map(|_| sampler(Driver::clone(&driver), Filter::Nearest))
        //         .collect(),
        // }
        todo!();
    }

    pub unsafe fn font(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::FONT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::FONT_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..80),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[
                    AttributeDesc {
                        binding: 0,
                        location: 0,
                        element: Element {
                            format: Format::Rg32Sfloat,
                            offset: 0,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 1,
                        element: Element {
                            format: Format::Rg32Sfloat,
                            offset: 8,
                        },
                    },
                ],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 16,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: None,
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn font_outline(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::FONT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::FONT_OUTLINE_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[
                (ShaderStageFlags::VERTEX, 0..64),
                (ShaderStageFlags::FRAGMENT, 64..96),
            ],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[
                    AttributeDesc {
                        binding: 0,
                        location: 0,
                        element: Element {
                            format: Format::Rg32Sfloat,
                            offset: 0,
                        },
                    },
                    AttributeDesc {
                        binding: 0,
                        location: 1,
                        element: Element {
                            format: Format::Rg32Sfloat,
                            offset: 8,
                        },
                    },
                ],
                buffers: &[VertexBufferDesc {
                    binding: 0,
                    stride: 16,
                    rate: VertexInputRate::Vertex,
                }],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn gradient(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::GRADIENT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::GRADIENT_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::FRAGMENT, 0..32)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn gradient_transparency(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::GRADIENT_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::GRADIENT_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::FRAGMENT, 0..32)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn present(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::QUAD_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::TEXTURE_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..64)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::ALPHA),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(Driver::clone(&driver), Filter::Nearest)],
        }
    }

    pub unsafe fn texture(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        subpass: Subpass<'_, _Backend>,
        max_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(Driver::clone(&driver), &spirv::QUAD_TRANSFORM_VERT);
        let fragment = ShaderModule::new(Driver::clone(&driver), &spirv::TEXTURE_FRAG);
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
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            once(&*set_layout),
            &[(ShaderStageFlags::VERTEX, 0..80)],
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: InputAssemblerDesc {
                    primitive: Primitive::TriangleList,
                    restart_index: None,
                    with_adjacency: false,
                },
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            FILL_RASTERIZER,
            Some(ShaderModule::entry_point(&fragment)),
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
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_sets,
            pipeline,
            set_layout: Some(set_layout),
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
        // TODO: Why the odd unwrap pattern twice here?
        unsafe {
            self.desc_pool.as_mut().unwrap().reset();
        }

        for desc_set in &mut self.desc_sets {
            *desc_set = unsafe {
                self.desc_pool
                    .as_mut()
                    .unwrap()
                    .allocate_set(self.set_layout.as_ref().unwrap())
                    .unwrap()
            }
        }
    }

    pub fn sampler(&self, idx: usize) -> &Sampler {
        &self.samplers[idx]
    }
}
