use {
    super::{desc_set_layout, push_const, push_const::ShaderRange, READ_ONLY_IMG, READ_WRITE_IMG},
    crate::{
        color::TRANSPARENT_BLACK,
        gpu::{
            device::{
                descriptor_range_desc, DescriptorPool, DescriptorSetLayout, Device,
                GraphicsPipeline, PipelineLayout, Sampler, ShaderModule,
            },
            spirv,
        },
    },
    gfx_hal::{
        image::{Filter, Lod, WrapMode},
        pass::Subpass,
        pso::{
            BlendState, ColorBlendDesc, ColorMask, Comparison, DepthTest, DescriptorPool as _,
            GraphicsPipelineDesc, LogicOp, PrimitiveAssemblerDesc, VertexBufferDesc,
            VertexInputRate,
        },
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

mod attributes {
    use gfx_hal::{
        format::Format,
        pso::{AttributeDesc, Element},
    };

    pub const VEC2_VEC2: [AttributeDesc; 2] = [
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
    ];
    pub const VEC3: [AttributeDesc; 1] = [AttributeDesc {
        binding: 0,
        location: 0,
        element: Element {
            format: Format::Rgb32Sfloat,
            offset: 0,
        },
    }];
    pub const VEC3_VEC2: [AttributeDesc; 2] = [
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
                format: Format::Rg32Sfloat,
                offset: 12,
            },
        },
    ];
    pub const VEC3_VEC3: [AttributeDesc; 2] = [
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
    ];
    pub const VEC3_VEC3_VEC4_VEC2: [AttributeDesc; 4] = [
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
                offset: 40,
            },
        },
    ];
    pub const VEC3_VEC4: [AttributeDesc; 2] = [
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
    ];
}

mod input_assemblers {
    use gfx_hal::pso::{InputAssemblerDesc, Primitive};

    pub const LINES: InputAssemblerDesc = InputAssemblerDesc {
        primitive: Primitive::LineList,
        restart_index: None,
        with_adjacency: false,
    };
    pub const TRIANGLES: InputAssemblerDesc = InputAssemblerDesc {
        primitive: Primitive::TriangleList,
        restart_index: None,
        with_adjacency: false,
    };
}

mod rasterizers {
    use gfx_hal::pso::{Face, FrontFace, PolygonMode, Rasterizer, State};

    pub const FILL: Rasterizer = Rasterizer {
        conservative: false,
        cull_face: Face::BACK,
        depth_bias: None,
        depth_clamping: false,
        front_face: FrontFace::CounterClockwise,
        line_width: State::Static(1.0),
        polygon_mode: PolygonMode::Fill,
    };
    pub const LINE: Rasterizer = Rasterizer {
        conservative: false,
        cull_face: Face::NONE,
        depth_bias: None,
        depth_clamping: false,
        front_face: FrontFace::Clockwise,
        line_width: State::Static(1.0),
        polygon_mode: PolygonMode::Line,
    };
}

fn sampler(device: Device, filter: Filter) -> Sampler {
    Sampler::new(
        device,
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

fn vertex_buf_with_stride(stride: u32) -> [VertexBufferDesc; 1] {
    [VertexBufferDesc {
        binding: 0,
        stride,
        rate: VertexInputRate::Vertex,
    }]
}

pub struct Graphics {
    desc_pool: Option<DescriptorPool>,
    desc_sets: Vec<<_Backend as Backend>::DescriptorSet>,
    layout: PipelineLayout,
    max_desc_sets: usize,
    pipeline: GraphicsPipeline,
    samplers: Vec<Sampler>,
    set_layout: Option<DescriptorSetLayout>,
}

impl Graphics {
    unsafe fn blend(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        max_desc_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(device, &spirv::blend::quad_transform_vert::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::BLEND,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::BLEND,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(2 * max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn blend_add(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::add_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_alpha_add(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::alpha_add_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_color_burn(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::color_burn_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_color_dodge(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::color_dodge_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_color(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::color_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_darken(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::darken_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_darker_color(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::darker_color_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_difference(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::difference_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_divide(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::divide_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_exclusion(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::exclusion_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_hard_light(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::hard_light_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_hard_mix(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::hard_mix_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_linear_burn(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::linear_burn_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_multiply(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::multiply_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_normal(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::normal_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_overlay(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::overlay_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_screen(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::screen_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_subtract(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::subtract_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn blend_vivid_light(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::blend(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::blend::vivid_light_frag::MAIN,
            max_desc_sets,
        )
    }

    unsafe fn draw_light(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        push_consts: &[ShaderRange],
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::defer::light_vert::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            empty::<&<_Backend as Backend>::DescriptorSetLayout>(),
            push_consts,
        );
        let vertex_buf = vertex_buf_with_stride(12);
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &attributes::VEC3,
                buffers: &vertex_buf,
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        Self {
            desc_pool: None,
            desc_sets: vec![],
            layout,
            max_desc_sets: 0,
            pipeline,
            set_layout: None,
            samplers: vec![],
        }
    }

    pub unsafe fn draw_line(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        debug_assert_eq!(max_desc_sets, 0);

        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::defer::line_vert::MAIN);
        let fragment = ShaderModule::new(device, &spirv::defer::line_frag::MAIN);
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            empty::<&<_Backend as Backend>::DescriptorSetLayout>(),
            &push_const::VERTEX_MAT4,
        );
        let vertex_buf = vertex_buf_with_stride(32);
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &attributes::VEC3_VEC4,
                buffers: &vertex_buf,
                geometry: None,
                input_assembler: input_assemblers::LINES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::LINE,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.logic_op = Some(LogicOp::Set);
        for _ in 0..4 {
            desc.blender.targets.push(ColorBlendDesc {
                blend: None,
                mask: ColorMask::ALL,
            });
        }
        let pipeline = GraphicsPipeline::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        Self {
            desc_pool: None,
            desc_sets: vec![],
            layout,
            max_desc_sets: 0,
            pipeline,
            set_layout: None,
            samplers: vec![],
        }
    }

    pub unsafe fn draw_mesh(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::defer::mesh_vert::MAIN);
        let fragment = ShaderModule::new(device, &spirv::defer::mesh_frag::MAIN);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::DRAW_MESH,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::VERTEX_MAT4,
        );
        let vertex_buf = vertex_buf_with_stride(48);
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &attributes::VEC3_VEC3_VEC4_VEC2,
                buffers: &vertex_buf,
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(3 * max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: (0..3).map(|_| sampler(device, Filter::Nearest)).collect(),
        }
    }

    pub unsafe fn draw_point_light(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        debug_assert_eq!(max_desc_sets, 0);

        Self::draw_light(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::defer::point_light_frag::MAIN,
            &push_const::DRAW_POINT_LIGHT,
        )
    }

    pub unsafe fn draw_rect_light(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        debug_assert_eq!(max_desc_sets, 0);

        Self::draw_light(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::defer::rect_light_frag::MAIN,
            &push_const::DRAW_RECT_LIGHT,
        )
    }

    pub unsafe fn draw_spotlight(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        debug_assert_eq!(max_desc_sets, 0);

        Self::draw_light(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::defer::spotlight_frag::MAIN,
            &push_const::DRAW_SPOTLIGHT,
        )
    }

    pub unsafe fn draw_sunlight(
        #[cfg(feature = "debug-names")] _name: &str,
        _device: Device,
        _subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        debug_assert_eq!(max_desc_sets, 0);

        todo!();
    }

    unsafe fn font(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        push_consts: &[ShaderRange],
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::font_vert::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::SINGLE_READ_ONLY_IMG,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            push_consts,
        );
        let vertex_buf = vertex_buf_with_stride(16);
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &attributes::VEC2_VEC2,
                buffers: &vertex_buf,
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn font_normal(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::font(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::font_frag::MAIN,
            &push_const::FONT,
            max_desc_sets,
        )
    }

    pub unsafe fn font_outline(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::font(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::font_outline_frag::MAIN,
            &push_const::FONT_OUTLINE,
            max_desc_sets,
        )
    }

    unsafe fn gradient(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::gradient_frag::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::SINGLE_READ_ONLY_IMG,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::VERTEX_MAT4,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn gradient_linear_trans(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::gradient(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::gradient_trans_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn gradient_linear(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::gradient(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::gradient_frag::MAIN,
            max_desc_sets,
        )
    }

    unsafe fn mask(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        max_desc_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(device, &spirv::blend::quad_transform_vert::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::BLEND,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::BLEND,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(2 * max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn mask_add(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::add_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn mask_darken(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::darken_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn mask_difference(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::difference_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn mask_intersect(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::intersect_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn mask_lighten(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::lighten_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn mask_subtract(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::mask(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::mask::subtract_frag::MAIN,
            max_desc_sets,
        )
    }

    unsafe fn matte(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        fragment_spirv: &[u32],
        max_desc_sets: usize,
    ) -> Self {
        let vertex = ShaderModule::new(device, &spirv::blend::quad_transform_vert::MAIN);
        let fragment = ShaderModule::new(device, fragment_spirv);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::BLEND,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::BLEND,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(2 * max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn matte_alpha(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::matte(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::matte::alpha_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn matte_alpha_inv(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::matte(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::matte::alpha_inv_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn matte_luma(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::matte(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::matte::luma_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn matte_luma_inv(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        Self::matte(
            #[cfg(feature = "debug-names")]
            name,
            device,
            subpass,
            &spirv::matte::luma_inv_frag::MAIN,
            max_desc_sets,
        )
    }

    pub unsafe fn present(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::quad_vert::MAIN);
        let fragment = ShaderModule::new(device, &spirv::texture_frag::MAIN);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::SINGLE_READ_ONLY_IMG,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::VERTEX_MAT4,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.targets.push(ColorBlendDesc {
            blend: Some(BlendState::ALPHA),
            mask: ColorMask::ALL,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(max_desc_sets, READ_WRITE_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub unsafe fn skydome(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::skydome_vert::MAIN);
        let fragment = ShaderModule::new(device, &spirv::skydome_frag::MAIN);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::SKYDOME,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::SKYDOME,
        );
        let vertex_buf = vertex_buf_with_stride(12);
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &attributes::VEC3,
                buffers: &vertex_buf,
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
            Some(ShaderModule::entry_point(&fragment)),
            &layout,
            subpass,
        );
        desc.blender.logic_op = None;
        desc.blender.targets.push(ColorBlendDesc {
            blend: None,
            mask: ColorMask::COLOR,
        });
        desc.depth_stencil.depth = Some(DepthTest {
            fun: Comparison::LessEqual,
            write: true,
        });
        let pipeline = GraphicsPipeline::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(6 * max_desc_sets, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: (0..6).map(|_| sampler(device, Filter::Nearest)).collect(),
        }
    }

    pub unsafe fn texture(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        subpass: Subpass<'_, _Backend>,
        max_desc_sets: usize,
    ) -> Self {
        // Create the graphics pipeline
        let vertex = ShaderModule::new(device, &spirv::quad_transform_vert::MAIN);
        let fragment = ShaderModule::new(device, &spirv::texture_frag::MAIN);
        let set_layout = DescriptorSetLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc_set_layout::SINGLE_READ_ONLY_IMG,
        );
        let layout = PipelineLayout::new(
            #[cfg(feature = "debug-names")]
            name,
            device,
            once(set_layout.as_ref()),
            &push_const::TEXTURE,
        );
        let mut desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                attributes: &[],
                buffers: &[],
                geometry: None,
                input_assembler: input_assemblers::TRIANGLES,
                tessellation: None,
                vertex: ShaderModule::entry_point(&vertex),
            },
            rasterizers::FILL,
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
            #[cfg(feature = "debug-names")]
            name,
            device,
            &desc,
        );

        // Allocate all descriptor sets
        let mut desc_pool = DescriptorPool::new(
            device,
            max_desc_sets,
            once(descriptor_range_desc(1, READ_ONLY_IMG)),
        );
        let layouts = (0..max_desc_sets).map(|_| set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);
        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        Self {
            desc_pool: Some(desc_pool),
            desc_sets,
            layout,
            max_desc_sets,
            pipeline,
            set_layout: Some(set_layout),
            samplers: vec![sampler(device, Filter::Nearest)],
        }
    }

    pub fn desc_set(&self, idx: usize) -> &<_Backend as Backend>::DescriptorSet {
        &self.desc_sets[idx]
    }

    pub fn layout(&self) -> &PipelineLayout {
        &self.layout
    }

    pub fn max_desc_sets(&self) -> usize {
        self.max_desc_sets
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
