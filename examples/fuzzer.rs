/*

Kind of an example, kind of a test - not good looking
Used for code coverage with https://github.com/mozilla/grcov

First time:
    rustup component add llvm-tools-preview

In a separate terminal:
    export RUSTFLAGS="-Cinstrument-coverage"
    cargo build --example fuzzer

Next:
    export LLVM_PROFILE_FILE="fuzzer-%p-%m.profraw"
    target/debug/examples/fuzzer


Also helpful to run with valgrind:
    cargo build --example fuzzer && valgrind target/debug/examples/fuzzer

*/
use {
    inline_spirv::inline_spirv,
    rand::{seq::SliceRandom, thread_rng},
    screen_13::prelude::*,
    std::{mem::size_of, sync::Arc},
};

type Operation = fn(&mut FrameContext, &mut HashPool);

const FRAME_COUNT: usize = 10;
const OPERATIONS_PER_FRAME: usize = 16;

static OPERATIONS: &[Operation] = &[
    record_compute_array_bind,
    record_compute_bindless,
    record_compute_no_op,
    record_graphic_bindless,
    record_graphic_load_store,
    record_graphic_msaa_depth_stencil,
    record_graphic_will_merge_subpass_input,
    record_graphic_wont_merge,
    record_accel_struct_builds,
];

fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let mut rng = thread_rng();

    // If ray tracing is unsupported then set that to false and remove the associated operations
    let screen_13 = EventLoop::new().debug(true).ray_tracing(true).build()?;
    let mut pool = HashPool::new(&screen_13.device);

    let mut frame_count = 0;

    screen_13.run(|mut frame| {
        // We stop fuzzing after 10 frames
        frame_count += 1;
        if frame_count == FRAME_COUNT {
            *frame.will_exit = true;
        }

        // We fuzz a set amount of randomly selected operations per frame
        for _ in 0..OPERATIONS_PER_FRAME {
            OPERATIONS.choose(&mut rng).unwrap()(&mut frame, &mut pool);
        }

        // We are not testing the swapchain - so always clear it
        frame.render_graph.clear_color_image(frame.swapchain_image);
    })?;

    debug!("OK");

    Ok(())
}

fn record_accel_struct_builds(frame: &mut FrameContext, pool: &mut HashPool) {
    const BLAS_COUNT: vk::DeviceSize = 32;

    // Vertex buffer for a triangle
    let vertex_buf = {
        let mut buf = pool
            .lease(BufferInfo::new_mappable(
                36,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ))
            .unwrap();

        // Vertex 1
        Buffer::copy_from_slice(&mut buf, 0, &0f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 4, &0f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 8, &0f32.to_ne_bytes());

        // Vertex 2
        Buffer::copy_from_slice(&mut buf, 12, &1f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 16, &1f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 20, &0f32.to_ne_bytes());

        // Vertex 3
        Buffer::copy_from_slice(&mut buf, 24, &2f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 28, &0f32.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 32, &0f32.to_ne_bytes());

        buf
    };

    // Index buffer for a single triangle
    let index_buf = {
        let mut buf = pool
            .lease(BufferInfo::new_mappable(
                6,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
            ))
            .unwrap();

        Buffer::copy_from_slice(&mut buf, 0, &0u16.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 2, &1u16.to_ne_bytes());
        Buffer::copy_from_slice(&mut buf, 4, &2u16.to_ne_bytes());

        buf
    };

    let blas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            max_primitive_count: 1,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Triangles {
                index_data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(&index_buf)),
                index_type: vk::IndexType::UINT16,
                max_vertex: 3,
                transform_data: None,
                vertex_data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(
                    &vertex_buf,
                )),
                vertex_format: vk::Format::R32G32B32_SFLOAT,
                vertex_stride: 12,
            },
        }],
    };
    let blas_size = AccelerationStructure::size_of(frame.device, &blas_geometry_info);
    let blas_info = AccelerationStructureInfo::new_blas(blas_size.create_size);

    let instance_len = size_of::<vk::AccelerationStructureInstanceKHR>() as vk::DeviceSize;
    let mut instance_buf = Buffer::create(
        frame.device,
        BufferInfo::new_mappable(
            instance_len * BLAS_COUNT,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        ),
    )
    .unwrap();

    // Lease and bind a bunch of bottom-level acceleration structures and add to instance buffer
    let mut blas_nodes = Vec::with_capacity(BLAS_COUNT as _);
    for idx in 0..BLAS_COUNT {
        let blas = pool.lease(blas_info).unwrap();

        Buffer::copy_from_slice(
            &mut instance_buf,
            idx * instance_len,
            AccelerationStructure::instance_slice(vk::AccelerationStructureInstanceKHR {
                transform: vk::TransformMatrixKHR {
                    matrix: [
                        1.0, 0.0, 0.0, 0.0, //
                        0.0, 1.0, 0.0, 0.0, //
                        0.0, 0.0, 1.0, 0.0, //
                    ],
                },
                instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xff),
                instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(
                    0,
                    vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE.as_raw() as _,
                ),
                acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
                    device_handle: AccelerationStructure::device_address(&blas),
                },
            }),
        );

        let blas_node = frame.render_graph.bind_node(blas);
        let scratch_buf = frame.render_graph.bind_node(
            pool.lease(BufferInfo::new(
                blas_size.build_size,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
            ))
            .unwrap(),
        );

        blas_nodes.push((scratch_buf, blas_node));
    }

    // Lease and bind a single top-level acceleration structure
    let tlas_geometry_info = AccelerationStructureGeometryInfo {
        ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
        flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
        geometries: vec![AccelerationStructureGeometry {
            max_primitive_count: 1,
            flags: vk::GeometryFlagsKHR::OPAQUE,
            geometry: AccelerationStructureGeometryData::Instances {
                array_of_pointers: false,
                data: DeviceOrHostAddress::DeviceAddress(Buffer::device_address(&instance_buf)),
            },
        }],
    };
    let tlas_size = AccelerationStructure::size_of(frame.device, &tlas_geometry_info);
    let tlas = pool
        .lease(AccelerationStructureInfo {
            ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
            size: tlas_size.create_size,
        })
        .unwrap();
    let tlas_node = frame.render_graph.bind_node(tlas);
    let tlas_scratch_buf = frame.render_graph.bind_node(
        pool.lease(BufferInfo::new(
            tlas_size.build_size,
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
        ))
        .unwrap(),
    );

    let index_node = frame.render_graph.bind_node(index_buf);
    let vertex_node = frame.render_graph.bind_node(vertex_buf);

    let pass = frame
        .render_graph
        .begin_pass("build acceleration structures");

    // TODO: AccessType for these is funky, should be access_node?
    let mut pass = pass.read_node(index_node).read_node(vertex_node);

    // TODO: Like this:
    for (scratch_buf, blas_node) in &blas_nodes {
        pass.access_node_mut(*scratch_buf, AccessType::AccelerationStructureBufferWrite);
        pass.access_node_mut(*blas_node, AccessType::AccelerationStructureBuildWrite);
    }

    // Ugly copy of the nodes that I want to figure out a way around while not being confusing
    let blas_nodes_copy = blas_nodes
        .iter()
        .map(|(_, blas_node)| *blas_node)
        .collect::<Vec<_>>();

    let mut pass = pass.record_acceleration(move |accel, _| {
        for (scratch_buf, blas_node) in blas_nodes {
            accel.build_structure(
                blas_node,
                scratch_buf,
                &blas_geometry_info,
                &[vk::AccelerationStructureBuildRangeInfoKHR {
                    first_vertex: 0,
                    primitive_count: 1,
                    primitive_offset: 0,
                    transform_offset: 0,
                }],
            );
        }
    });

    for blas_node in blas_nodes_copy {
        pass.access_node_mut(blas_node, AccessType::AccelerationStructureBuildRead);
    }

    pass.access_node_mut(
        tlas_scratch_buf,
        AccessType::AccelerationStructureBufferWrite,
    );
    pass.access_node_mut(tlas_node, AccessType::AccelerationStructureBuildWrite);

    pass.record_acceleration(move |accel, _| {
        accel.build_structure(
            tlas_node,
            tlas_scratch_buf,
            &tlas_geometry_info,
            &[vk::AccelerationStructureBuildRangeInfoKHR {
                first_vertex: 0,
                primitive_count: 1,
                primitive_offset: 0,
                transform_offset: 0,
            }],
        );
    });
}

fn record_compute_array_bind(frame: &mut FrameContext, pool: &mut HashPool) {
    let pipeline = compute_pipeline(
        "array_bind",
        frame.device,
        ComputePipelineInfo::new(
            inline_spirv!(
                r#"
                #version 460 core
                
                layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
                
                layout(constant_id = 0) const uint LAYER_COUNT = 1;
                
                layout(push_constant) uniform PushConstants {
                    layout(offset = 0) float offset;
                } push_const;
                
                layout(set = 0, binding = 0) uniform sampler2D layer_images_sampler_llr[LAYER_COUNT];
                
                void main() {
                }
                "#,
                comp
            )
            .as_slice(),
        )
        .specialization_info(SpecializationInfo::new(
            vec![vk::SpecializationMapEntry {
                constant_id: 0,
                offset: 0,
                size: 4,
            }],
            5u32.to_ne_bytes(),
        )),
    );

    let image_info = ImageInfo::new_2d(
        vk::Format::R8G8B8A8_UNORM,
        64,
        64,
        vk::ImageUsageFlags::SAMPLED,
    )
    .build();
    let images = [
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
    ];

    frame
        .render_graph
        .begin_pass("no-op")
        .bind_pipeline(&pipeline)
        .read_descriptor((0, [0]), images[0])
        .read_descriptor((0, [1]), images[1])
        .read_descriptor((0, [2]), images[2])
        .read_descriptor((0, [3]), images[3])
        .read_descriptor((0, [4]), images[4])
        .record_compute(|compute, _| {
            compute
                .push_constants(&0f32.to_ne_bytes())
                .dispatch(64, 64, 1);
        });
}

fn record_compute_bindless(frame: &mut FrameContext, pool: &mut HashPool) {
    let pipeline = compute_pipeline(
        "bindless",
        frame.device,
        ComputePipelineInfo::new(
            inline_spirv!(
                r#"
                #version 460 core
                #extension GL_EXT_nonuniform_qualifier : require
                
                layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

                layout(push_constant) uniform PushConstants {
                    layout(offset = 0) uint count;
                } push_const;
                
                layout(set = 0, binding = 0, rgba8) writeonly uniform image2D dst[];
                
                void main() {
                    for (uint idx = 0; idx < push_const.count; idx++) {
                        imageStore(
                            dst[idx],
                            ivec2(gl_GlobalInvocationID.x, gl_GlobalInvocationID.y),
                            vec4(0)
                        );
                    }
                }
                "#,
                comp
            )
            .as_slice(),
        ),
    );

    let image_info = ImageInfo::new_2d(
        vk::Format::R8G8B8A8_UNORM,
        64,
        64,
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE,
    )
    .build();
    let images = [
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
    ];

    frame
        .render_graph
        .begin_pass("no-op")
        .bind_pipeline(&pipeline)
        .write_descriptor((0, [0]), images[0])
        .write_descriptor((0, [1]), images[1])
        .write_descriptor((0, [2]), images[2])
        .write_descriptor((0, [3]), images[3])
        .write_descriptor((0, [4]), images[4])
        .record_compute(|compute, _| {
            compute
                .push_constants(&5u32.to_ne_bytes())
                .dispatch(64, 64, 1);
        });
}

fn record_compute_no_op(frame: &mut FrameContext, _: &mut HashPool) {
    let pipeline = compute_pipeline(
        "no_op",
        frame.device,
        inline_spirv!(
            r#"
            #version 460 core

            void main() {
            }
            "#,
            comp
        )
        .as_slice(),
    );
    frame
        .render_graph
        .begin_pass("no-op")
        .bind_pipeline(&pipeline)
        .record_compute(|compute, _| {
            compute.dispatch(1, 1, 1);
        });
}

fn record_graphic_bindless(frame: &mut FrameContext, pool: &mut HashPool) {
    let pipeline = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        inline_spirv!(
            r#"
            #version 460 core
    
            void main() {
            }
            "#,
            vert
        )
        .as_slice(),
        inline_spirv!(
            r#"
            #version 460 core
            #extension GL_EXT_nonuniform_qualifier : require

            layout(push_constant) uniform PushConstants {
                layout(offset = 0) uint count;
            } push_const;

            layout(set = 0, binding = 0) uniform sampler2D images_sampler_llr[];

            layout(location = 0) out vec4 color_out;

            void main() {
                for (uint idx = 0; idx < push_const.count; idx++) {
                    color_out += texture(
                        images_sampler_llr[idx],
                        vec2(0)
                    );
                }
            }
            "#,
            frag
        )
        .as_slice(),
    );

    let image = frame.render_graph.bind_node(
        pool.lease(ImageInfo::new_2d(
            vk::Format::R8G8B8A8_UNORM,
            256,
            256,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT,
        ))
        .unwrap(),
    );
    let image_info = ImageInfo::new_2d(
        vk::Format::R8G8B8A8_UNORM,
        64,
        64,
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE,
    )
    .build();
    let images = [
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
        frame
            .render_graph
            .bind_node(pool.lease(image_info).unwrap()),
    ];

    frame
        .render_graph
        .begin_pass("a")
        .bind_pipeline(&pipeline)
        .read_descriptor((0, [0]), images[0])
        .read_descriptor((0, [1]), images[1])
        .read_descriptor((0, [2]), images[2])
        .read_descriptor((0, [3]), images[3])
        .read_descriptor((0, [4]), images[4])
        .clear_color(0, image)
        .store_color(0, image)
        .record_subpass(|subpass, _| {
            subpass.push_constants(&5u32.to_ne_bytes()).draw(1, 1, 0, 0);
        });
}

fn record_graphic_load_store(frame: &mut FrameContext, _: &mut HashPool) {
    let pipeline = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        inline_spirv!(
            r#"
            #version 460 core
    
            void main() {
            }
            "#,
            vert
        )
        .as_slice(),
        inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) out vec4 color_out;

            void main() {
                color_out = vec4(0);
            }
            "#,
            frag
        )
        .as_slice(),
    );

    frame
        .render_graph
        .begin_pass("load-store")
        .bind_pipeline(&pipeline)
        .load_color(0, frame.swapchain_image)
        .store_color(0, frame.swapchain_image)
        .record_subpass(|subpass, _| {
            subpass.draw(1, 1, 0, 0);
        });
}

fn record_graphic_msaa_depth_stencil(frame: &mut FrameContext, pool: &mut HashPool) {
    let pipeline = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::new().samples(SampleCount::X8),
        inline_spirv!(
            r#"
            #version 460 core

            const vec2 UV[3] = {
                vec2(-1, -1),
                vec2(-1, 1),
                vec2(1, 1),
            };

            void main() {
                gl_Position = vec4(UV[gl_VertexIndex], 0, 1);
            }
            "#,
            vert
        )
        .as_slice(),
        inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) out vec4 color_out;

            void main() {
                color_out = vec4(1);
            }
            "#,
            frag
        )
        .as_slice(),
    );

    let swapchain_format = frame.render_graph.node_info(frame.swapchain_image).fmt;
    let msaa_color_image = frame.render_graph.bind_node(
        pool.lease(
            ImageInfo::new_2d(
                swapchain_format,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            )
            .sample_count(SampleCount::X8),
        )
        .unwrap(),
    );
    let msaa_depth_stencil_image = frame.render_graph.bind_node(
        pool.lease(
            ImageInfo::new_2d(
                vk::Format::D24_UNORM_S8_UINT,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            )
            .sample_count(SampleCount::X8),
        )
        .unwrap(),
    );
    let depth_stencil_image = frame.render_graph.bind_node(
        pool.lease(ImageInfo::new_2d(
            vk::Format::D24_UNORM_S8_UINT,
            frame.width,
            frame.height,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        ))
        .unwrap(),
    );

    let depth_stencil_mode = DepthStencilMode {
        back: StencilMode::IGNORE,
        bounds_test: true,
        compare_op: vk::CompareOp::LESS_OR_EQUAL,
        depth_test: true,
        depth_write: true,
        front: StencilMode {
            fail_op: vk::StencilOp::ZERO,
            pass_op: vk::StencilOp::REPLACE,
            depth_fail_op: vk::StencilOp::ZERO,
            compare_op: vk::CompareOp::LESS_OR_EQUAL,
            compare_mask: 0xff,
            write_mask: 0xff,
            reference: 0x00,
        },
        min: 0.0.into(),
        max: 1.0.into(),
        stencil_test: true,
    };

    frame
        .render_graph
        .begin_pass("msaa-depth-stencil")
        .bind_pipeline(&pipeline)
        .set_depth_stencil(depth_stencil_mode)
        .clear_color(0, msaa_color_image)
        .clear_depth_stencil(msaa_depth_stencil_image)
        .resolve_color(0, 1, frame.swapchain_image)
        .resolve_depth_stencil(
            2,
            depth_stencil_image,
            Some(ResolveMode::Average),
            Some(ResolveMode::SampleZero),
        )
        .record_subpass(|subpass, _| {
            subpass.draw(3, 1, 0, 0);
        });
}

fn record_graphic_will_merge_subpass_input(frame: &mut FrameContext, pool: &mut HashPool) {
    let vertex = inline_spirv!(
        r#"
        #version 460 core

        void main() {
        }
        "#,
        vert
    )
    .as_slice();
    let pipeline_a = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        vertex,
        inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) out vec4 color_out;

            void main() {
                color_out = vec4(0);
            }
            "#,
            frag
        )
        .as_slice(),
    );
    let pipeline_b = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        vertex,
        inline_spirv!(
            r#"
            #version 460 core

            layout(input_attachment_index = 0, binding = 0) uniform subpassInput color_in;
            layout(location = 0) out vec4 color_out;

            void main() {
                color_out = subpassLoad(color_in);
            }
            "#,
            frag
        )
        .as_slice(),
    );
    let image = frame.render_graph.bind_node(
        pool.lease(ImageInfo::new_2d(
            vk::Format::R8G8B8A8_UNORM,
            256,
            256,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT,
        ))
        .unwrap(),
    );

    // Pass "a" stores color 0 which "b" compatibly inputs "image"; so these two will get merged
    frame
        .render_graph
        .begin_pass("a")
        .bind_pipeline(&pipeline_a)
        .clear_color(0, image)
        .store_color(0, image)
        .record_subpass(|subpass, _| {
            subpass.draw(1, 1, 0, 0);
        });
    frame
        .render_graph
        .begin_pass("b")
        .bind_pipeline(&pipeline_b)
        .store_color(0, image)
        .record_subpass(|subpass, _| {
            subpass.draw(1, 1, 0, 0);
        });
}

fn record_graphic_wont_merge(frame: &mut FrameContext, pool: &mut HashPool) {
    let pipeline = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        inline_spirv!(
            r#"
            #version 460 core

            void main() {
            }
            "#,
            vert
        )
        .as_slice(),
        inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) out vec4 color;

            void main() {
            }
            "#,
            frag
        )
        .as_slice(),
    );

    let image = frame.render_graph.bind_node(
        pool.lease(ImageInfo::new_2d(
            vk::Format::R8G8B8A8_UNORM,
            256,
            256,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
        ))
        .unwrap(),
    );

    // These two passes have common writes but are otherwise regular - they won't get merged
    frame
        .render_graph
        .begin_pass("c")
        .bind_pipeline(&pipeline)
        .store_color(0, image)
        .record_subpass(|subpass, _| {
            subpass.draw(0, 0, 0, 0);
        });
    frame
        .render_graph
        .begin_pass("d")
        .bind_pipeline(&pipeline)
        .store_color(0, image)
        .record_subpass(|subpass, _| {
            subpass.draw(0, 0, 0, 0);
        });
}

// Below are convenience functions used to create test data

fn compute_pipeline(
    key: &'static str,
    device: &Arc<Device>,
    info: impl Into<ComputePipelineInfo>,
) -> Arc<ComputePipeline> {
    use std::{cell::RefCell, collections::HashMap};

    thread_local! {
        static TLS: RefCell<HashMap<&'static str, Arc<ComputePipeline>>> = Default::default();
    }

    TLS.with(|tls| {
        Arc::clone(
            tls.borrow_mut()
                .entry(key)
                .or_insert_with(|| Arc::new(ComputePipeline::create(device, info).unwrap())),
        )
    })
}

fn graphic_vert_frag_pipeline(
    device: &Arc<Device>,
    info: impl Into<GraphicPipelineInfo>,
    vert_source: &'static [u32],
    frag_source: &'static [u32],
) -> Arc<GraphicPipeline> {
    use std::{cell::RefCell, collections::HashMap};

    #[derive(Eq, Hash, PartialEq)]
    struct Key {
        info: GraphicPipelineInfo,
        vert_source: &'static [u32],
        frag_source: &'static [u32],
    }

    thread_local! {
        static TLS: RefCell<HashMap<Key, Arc<GraphicPipeline>>> = Default::default();
    }

    let info = info.into();

    TLS.with(|tls| {
        Arc::clone(
            tls.borrow_mut()
                .entry(Key {
                    info: info.clone(),
                    vert_source,
                    frag_source,
                })
                .or_insert_with(move || {
                    Arc::new(
                        GraphicPipeline::create(
                            device,
                            info,
                            [
                                Shader::new_vertex(vert_source),
                                Shader::new_fragment(frag_source),
                            ],
                        )
                        .unwrap(),
                    )
                }),
        )
    })
}
