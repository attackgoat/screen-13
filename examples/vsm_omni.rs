use {
    bytemuck::{bytes_of, cast_slice, NoUninit, Pod, Zeroable},
    glam::{vec3, Mat4, Quat, Vec3},
    inline_spirv::inline_spirv,
    meshopt::remap::{generate_vertex_remap, remap_index_buffer, remap_vertex_buffer},
    screen_13::prelude::*,
    std::{
        env::current_exe,
        fs::{metadata, write},
        path::{Path, PathBuf},
        sync::Arc,
    },
    tobj::{load_obj, GPU_LOAD_OPTIONS},
};

const BLUR_PASSES: usize = 2;
const BLUR_RADIUS: u32 = 5;
const CUBEMAP_SIZE: u32 = 1024;
const SHADOW_BIAS: f32 = 0.3;

/// Adapted from https://github.com/sydneyzh/variance_shadow_mapping_vk
///
/// This example does an HTTPS GET to acquire model data!
/// Model data courtesy: https://github.com/alecjacobson/common-3d-test-models/
///
/// Also, see similar techniques here:
/// https://github.com/SaschaWillems/Vulkan/blob/ae3c1325f8c7a55941dc5b325db58bb482dce04c/examples/shadowmappingomni/shadowmappingomni.cpp
/// https://github.com/SaschaWillems/Vulkan/pull/783
fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let model_path = download_model_from_github("nefertiti.obj")?;
    let model_transform = Mat4::from_scale_rotation_translation(
        Vec3::splat(15.0),
        Quat::from_rotation_z(180f32.to_radians()) * Quat::from_rotation_x(90f32.to_radians()),
        vec3(0.0, -2.0, 0.0),
    )
    .to_cols_array();
    let cube_transform = Mat4::from_scale(Vec3::splat(10.0)).to_cols_array();

    let mut keyboard = KeyBuf::default();
    let event_loop = EventLoop::new()
        .window(|window| window.with_inner_size(LogicalSize::new(800, 600)))
        .build()?;

    // Load all the immutable graphics data we will need
    let model_mesh = load_model_mesh(&event_loop.device, &model_path)?;
    let model_shadow = load_model_shadow(&event_loop.device, &model_path)?;
    let cube_mesh = load_cube_mesh(&event_loop.device)?;
    let cube_shadow = load_cube_shadow(&event_loop.device)?;
    let debug_pipeline = create_debug_pipeline(&event_loop.device)?;
    let blur_x_pipeline = create_blur_x_pipeline(&event_loop.device)?;
    let blur_y_pipeline = create_blur_y_pipeline(&event_loop.device)?;
    let mesh_pipeline = create_mesh_pipeline(&event_loop.device)?;
    let shadow_pipeline = create_shadow_pipeline(&event_loop.device)?;

    // A pool will be used for per-frame resources
    let mut pool = LazyPool::new(&event_loop.device);

    let mut elapsed = 0.0;
    event_loop.run(|frame| {
        update_keyboard(&mut keyboard, frame.events);

        // Hold spacebar to stop the light
        if !keyboard.is_held(&VirtualKeyCode::Space) {
            elapsed += frame.dt;
        }

        // Calculate values for and fill some plain-old-data structs we will bind as UBO's
        let camera = {
            let aspect_ratio = frame.width as f32 / frame.height as f32;
            let fov_y = 45f32.to_radians();
            let projection = Mat4::perspective_rh(fov_y, aspect_ratio, 0.1, 100.0);

            let eye = vec3(0.0, 0.0, -25.0);
            let view = Mat4::look_at_rh(eye, eye + Vec3::Z, -Vec3::Y);

            Camera { view, projection }
        };
        let light = {
            let fov_y = 90f32.to_radians();
            let radius = 7f32;
            let t = elapsed / 2.0 + 3.5;
            let position = vec3(radius * t.cos(), 0.0, radius * t.sin());

            Light {
                position,
                range: 1000.0,
                view: Mat4::look_at_rh(position, position + Vec3::X, Vec3::Y),
                projection: Mat4::perspective_rh(fov_y, 1.0, 0.1, 100.0),
            }
        };

        // Bind resources to the render graph of the current frame
        let cube_mesh_index_buf = frame.render_graph.bind_node(&cube_mesh.index_buf);
        let cube_mesh_vertex_buf = frame.render_graph.bind_node(&cube_mesh.vertex_buf);
        let cube_shadow_index_buf = frame.render_graph.bind_node(&cube_shadow.index_buf);
        let cube_shadow_vertex_buf = frame.render_graph.bind_node(&cube_shadow.vertex_buf);
        let model_mesh_index_buf = frame.render_graph.bind_node(&model_mesh.index_buf);
        let model_mesh_vertex_buf = frame.render_graph.bind_node(&model_mesh.vertex_buf);
        let model_shadow_index_buf = frame.render_graph.bind_node(&model_shadow.index_buf);
        let model_shadow_vertex_buf = frame.render_graph.bind_node(&model_shadow.vertex_buf);
        let camera_uniform_buf = frame
            .render_graph
            .bind_node(lease_uniform_buffer(&mut pool, &camera).unwrap());
        let light_uniform_buf = frame
            .render_graph
            .bind_node(lease_uniform_buffer(&mut pool, &light).unwrap());

        // Lease and bind a cube-compatible shadow 2D image array to the graph of the current frame
        let shadow_faces_image = pool
            .lease(
                ImageInfo::new_2d_array(
                    vk::Format::R32G32_SFLOAT,
                    CUBEMAP_SIZE,
                    CUBEMAP_SIZE,
                    6,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::STORAGE,
                )
                .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE),
            )
            .unwrap();
        let shadow_faces_info = shadow_faces_image.info;
        let shadow_faces_node = frame.render_graph.bind_node(shadow_faces_image);

        // Lease and bind a temporary image we'll use during blur passes
        let temp_image = frame
            .render_graph
            .bind_node(pool.lease(shadow_faces_info).unwrap());

        // Lastly we lease and bind depth images needed for rendering
        let depth_cube = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d_array(
                vk::Format::D32_SFLOAT,
                frame.width,
                frame.height,
                6,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            ))
            .unwrap(),
        );
        let depth_image = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d(
                vk::Format::D32_SFLOAT,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            ))
            .unwrap(),
        );

        // Hold tab to view a debug mode
        if keyboard.is_held(&VirtualKeyCode::Tab) {
            frame
                .render_graph
                .begin_pass("DEBUG")
                .bind_pipeline(&debug_pipeline)
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(
                    0,
                    camera_uniform_buf,
                    AccessType::AnyShaderReadUniformBuffer,
                )
                .access_descriptor(1, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_node(model_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(model_mesh_vertex_buf, AccessType::VertexBuffer)
                .access_node(cube_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(cube_mesh_vertex_buf, AccessType::VertexBuffer)
                .clear_color(0, frame.swapchain_image)
                .store_color(0, frame.swapchain_image)
                .clear_depth_stencil(depth_image)
                .store_depth_stencil(depth_image)
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_mesh_vertex_buf)
                        .push_constants(cast_slice(&model_transform))
                        .draw_indexed(model_mesh.index_count, 1, 0, 0, 0)
                        .bind_index_buffer(cube_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(cube_mesh_vertex_buf)
                        .push_constants(cast_slice(&cube_transform))
                        .draw_indexed(cube_mesh.index_count, 1, 0, 0, 0);
                });
        } else {
            // Render the omni light point of view into the six-layer image we leased above
            frame
                .render_graph
                .begin_pass("Shadow")
                .bind_pipeline(&shadow_pipeline)
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(0, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_node(model_shadow_index_buf, AccessType::IndexBuffer)
                .access_node(model_shadow_vertex_buf, AccessType::VertexBuffer)
                .access_node(cube_shadow_index_buf, AccessType::IndexBuffer)
                .access_node(cube_shadow_vertex_buf, AccessType::VertexBuffer)
                .clear_color_value(0, shadow_faces_node, [light.range, light.range, 0.0, 0.0])
                .store_color(0, shadow_faces_node)
                .clear_depth_stencil(depth_cube)
                .store_depth_stencil(depth_cube) // TODO: Not required, bug
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_shadow_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_shadow_vertex_buf)
                        .push_constants(cast_slice(&model_transform))
                        .draw_indexed(model_shadow.index_count, 1, 0, 0, 0)
                        .bind_index_buffer(cube_shadow_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(cube_shadow_vertex_buf)
                        .push_constants(cast_slice(&cube_transform))
                        .draw_indexed(cube_shadow.index_count, 1, 0, 0, 0);
                });

            if BLUR_RADIUS > 0 {
                for _ in 0..BLUR_PASSES {
                    // Flip-flop between the shadow image and a temporary image using a
                    // separable box blur filter which approximates a gaussian blur
                    frame
                        .render_graph
                        .begin_pass("Blur X")
                        .bind_pipeline(&blur_x_pipeline)
                        .read_descriptor(0, shadow_faces_node)
                        .write_descriptor(1, temp_image)
                        .record_compute(move |compute, _| {
                            compute.dispatch(1, CUBEMAP_SIZE, 6);
                        })
                        .submit_pass()
                        .begin_pass("Blur Y")
                        .bind_pipeline(&blur_y_pipeline)
                        .read_descriptor(0, temp_image)
                        .write_descriptor(1, shadow_faces_node)
                        .record_compute(move |compute, _| {
                            compute.dispatch(CUBEMAP_SIZE, 1, 6);
                        });
                }
            }

            // Render the scene directly to the swapchain using the shadow map from the above pass
            frame
                .render_graph
                .begin_pass("Mesh objects")
                .bind_pipeline(&mesh_pipeline)
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(
                    0,
                    camera_uniform_buf,
                    AccessType::AnyShaderReadUniformBuffer,
                )
                .access_descriptor(1, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .read_descriptor_as(
                    2,
                    shadow_faces_node,
                    shadow_faces_info
                        .default_view_info()
                        .with_ty(ImageType::Cube),
                )
                .access_node(model_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(model_mesh_vertex_buf, AccessType::VertexBuffer)
                .access_node(cube_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(cube_mesh_vertex_buf, AccessType::VertexBuffer)
                .clear_color(0, frame.swapchain_image)
                .store_color(0, frame.swapchain_image)
                .clear_depth_stencil(depth_image)
                .store_depth_stencil(depth_image) // TODO: Not required, bug
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_mesh_vertex_buf)
                        .push_constants(cast_slice(&model_transform))
                        .draw_indexed(model_mesh.index_count, 1, 0, 0, 0)
                        .bind_index_buffer(cube_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(cube_mesh_vertex_buf)
                        .push_constants(cast_slice(&cube_transform))
                        .draw_indexed(cube_mesh.index_count, 1, 0, 0, 0);
                });
        }
    })?;

    Ok(())
}

fn create_blur_x_pipeline(device: &Arc<Device>) -> Result<Arc<ComputePipeline>, DriverError> {
    let comp = inline_spirv!(
        r#"
        #version 450 core

        #define POS_X 0
        #define NEG_X 1
        #define POS_Y 2
        #define NEG_Y 3
        #define POS_Z 4
        #define NEG_Z 5

        layout (local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

        layout(constant_id = 0) const uint IMAGE_SIZE = 512;
        layout(constant_id = 1) const uint RADIUS = 4;

        layout (set = 0, binding = 0, rg32f) restrict readonly uniform image2DArray image;
        layout (set = 0, binding = 1, rg32f) restrict writeonly uniform image2DArray image_out;

        ivec3 leading_face(uint x) {
            uint face = gl_GlobalInvocationID.z;
            uint y = gl_GlobalInvocationID.y;

            switch (face) {
                case POS_X:
                    return ivec3(x, y, POS_Z);
                case NEG_X:
                    return ivec3(x, y, NEG_Z);
                case POS_Y:
                    return ivec3(y, IMAGE_SIZE - (x + 1), NEG_X);
                case NEG_Y:
                    return ivec3(IMAGE_SIZE - (y + 1), x, NEG_X);
                case POS_Z:
                    return ivec3(x, y, NEG_X);
                default:
                    return ivec3(x, y, POS_X);
            }
        }

        ivec3 trailing_face(uint x) {
            uint face = gl_GlobalInvocationID.z;
            uint y = gl_GlobalInvocationID.y;

            switch (face) {
                case POS_X:
                    return ivec3(x, y, NEG_Z);
                case NEG_X:
                    return ivec3(x, y, POS_Z);
                case POS_Y:
                    return ivec3(IMAGE_SIZE - (y + 1), x, POS_X);
                case NEG_Y:
                    return ivec3(y, IMAGE_SIZE - (x + 1), POS_X);
                case POS_Z:
                    return ivec3(x, y, POS_X);
                default:
                    return ivec3(x, y, NEG_X);
            }
        }

        void main() {
            uint face = gl_GlobalInvocationID.z;
            uint y = gl_GlobalInvocationID.y;

            vec2 accumulator = vec2(0.0);
            float per_texel = 1.0 / float((RADIUS << 1) + 1);

            for (uint x = IMAGE_SIZE - RADIUS; x < IMAGE_SIZE; x++) {
                accumulator += imageLoad(image, leading_face(x)).rg;
            }

            for (uint x = 0; x < RADIUS; x++) {
                accumulator += imageLoad(image, ivec3(x, y, face)).rg;
            }

            for (uint x = 0; x < RADIUS; x++) {
                accumulator += imageLoad(image, ivec3(x + RADIUS, y, face)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, leading_face((IMAGE_SIZE - RADIUS) + x)).rg;
            }

            for (uint x = RADIUS; x < IMAGE_SIZE - RADIUS; x++) {
                accumulator += imageLoad(image, ivec3(x + RADIUS, y, face)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, ivec3(x - RADIUS, y, face)).rg;
            }

            for (uint x = IMAGE_SIZE - RADIUS; x < IMAGE_SIZE; x++) {
                accumulator += imageLoad(image, trailing_face((x + RADIUS) - IMAGE_SIZE)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, ivec3(x - RADIUS, y, face)).rg;
            }
        }
        "#,
        comp
    );

    let info =
        ComputePipelineInfo::new(comp.as_slice()).specialization_info(SpecializationInfo::new(
            vec![
                vk::SpecializationMapEntry {
                    constant_id: 0,
                    offset: 0,
                    size: 4,
                },
                vk::SpecializationMapEntry {
                    constant_id: 1,
                    offset: 4,
                    size: 4,
                },
            ],
            bytes_of(&Blur {
                image_size: CUBEMAP_SIZE,
                radius: BLUR_RADIUS,
            }),
        ));

    Ok(Arc::new(ComputePipeline::create(device, info)?))
}

fn create_blur_y_pipeline(device: &Arc<Device>) -> Result<Arc<ComputePipeline>, DriverError> {
    let comp = inline_spirv!(
        r#"
        #version 450 core

        #define POS_X 0
        #define NEG_X 1
        #define POS_Y 2
        #define NEG_Y 3
        #define POS_Z 4
        #define NEG_Z 5

        layout (local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

        layout(constant_id = 0) const uint IMAGE_SIZE = 512;
        layout(constant_id = 1) const uint RADIUS = 4;

        layout (set = 0, binding = 0, rg32f) restrict readonly uniform image2DArray image;
        layout (set = 0, binding = 1, rg32f) restrict writeonly uniform image2DArray image_out;

        ivec3 leading_face(uint y) {
            uint face = gl_GlobalInvocationID.z;
            uint x = gl_GlobalInvocationID.x;

            switch (face) {
                case POS_X:
                    return ivec3(y, IMAGE_SIZE - (x + 1), POS_Y);
                case NEG_X:
                    return ivec3(IMAGE_SIZE - (y + 1), x, POS_Y);
                case POS_Y:
                    return ivec3(IMAGE_SIZE - (x + 1), IMAGE_SIZE - (y + 1), NEG_Z);
                case NEG_Y:
                    return ivec3(x, y, POS_Z);
                case POS_Z:
                    return ivec3(x, y, POS_Y);
                default:
                    return ivec3(IMAGE_SIZE - (x + 1), IMAGE_SIZE - (y + 1), POS_Y);
            }
        }

        ivec3 trailing_face(uint y) {
            uint face = gl_GlobalInvocationID.z;
            uint x = gl_GlobalInvocationID.x;

            switch (face) {
                case POS_X:
                    return ivec3(IMAGE_SIZE - (y + 1), x, NEG_Y);
                case NEG_X:
                    return ivec3(y, IMAGE_SIZE - (x + 1), NEG_Y);
                case POS_Y:
                    return ivec3(x, y, POS_Z);
                case NEG_Y:
                    return ivec3(IMAGE_SIZE - (x + 1), IMAGE_SIZE - (y + 1), NEG_Z);
                case POS_Z:
                    return ivec3(x, y, NEG_Y);
                default:
                    return ivec3(IMAGE_SIZE - (x + 1), IMAGE_SIZE - (y + 1), NEG_Y);
            }
        }

        void main() {
            uint face = gl_GlobalInvocationID.z;
            uint x = gl_GlobalInvocationID.x;

            vec2 accumulator = vec2(0.0);
            float per_texel = 1.0 / float((RADIUS << 1) + 1);

            for (uint y = IMAGE_SIZE - RADIUS; y < IMAGE_SIZE; y++) {
                accumulator += imageLoad(image, leading_face(y)).rg;
            }

            for (uint y = 0; y < RADIUS; y++) {
                accumulator += imageLoad(image, ivec3(x, y, face)).rg;
            }

            for (uint y = 0; y < RADIUS; y++) {
                accumulator += imageLoad(image, ivec3(x, y + RADIUS, face)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, leading_face((IMAGE_SIZE - RADIUS) + y)).rg;
            }

            for (uint y = RADIUS; y < IMAGE_SIZE - RADIUS; y++) {
                accumulator += imageLoad(image, ivec3(x, y + RADIUS, face)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, ivec3(x, y - RADIUS, face)).rg;
            }

            for (uint y = IMAGE_SIZE - RADIUS; y < IMAGE_SIZE; y++) {
                accumulator += imageLoad(image, trailing_face((y + RADIUS) - IMAGE_SIZE)).rg;
                imageStore(image_out, ivec3(x, y, face), vec4(accumulator * per_texel, 0.0, 0.0));
                accumulator -= imageLoad(image, ivec3(x, y - RADIUS, face)).rg;
            }
        }
        "#,
        comp
    );

    let info =
        ComputePipelineInfo::new(comp.as_slice()).specialization_info(SpecializationInfo::new(
            vec![
                vk::SpecializationMapEntry {
                    constant_id: 0,
                    offset: 0,
                    size: 4,
                },
                vk::SpecializationMapEntry {
                    constant_id: 1,
                    offset: 4,
                    size: 4,
                },
            ],
            bytes_of(&Blur {
                image_size: CUBEMAP_SIZE,
                radius: BLUR_RADIUS,
            }),
        ));

    Ok(Arc::new(ComputePipeline::create(device, info)?))
}

fn create_debug_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 450 core

        layout(push_constant) uniform PushConstants {
            layout(offset = 0) mat4 model;
        } push_const;

        layout(set = 0, binding = 0) uniform Camera {
            mat4 view;
            mat4 projection;
        } camera;

        layout(location = 0) in vec3 position;
        layout(location = 1) in vec3 normal;

        layout(location = 0) out vec3 world_position_out;
        layout(location = 1) out vec3 world_normal_out;

        void main()
        {
            world_position_out = (push_const.model * vec4(position, 1)).xyz;
            world_normal_out = normalize((push_const.model * vec4(normal, 1)).xyz);
            gl_Position = camera.projection * camera.view * vec4(world_position_out, 1);
        }
        "#,
        vert
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core

        layout(set = 0, binding = 1) uniform Light {
            vec3 position;
            float range;
            mat4 view0;
            mat4 projection;
        } light;

        layout(location = 0) in vec3 world_position;
        layout(location = 1) in vec3 world_normal;

        layout(location = 0) out vec4 color_out;

        void main()
        {
            color_out = vec4(vec3(0.0), 1.0);

            vec3 light_dir = light.position - world_position.xyz;
            float light_dist = length(light_dir);

            if (light_dist < light.range)
            {
                light_dir = normalize(light_dir);

                float lambertian = max(0.0, dot(world_normal, light_dir));
                float attenuation = max(0.0, min(1.0, light_dist / light.range));
                attenuation = 1.0 - attenuation * attenuation;

                color_out.rgb = vec3(lambertian * attenuation);
            }
        }
        "#,
        frag
    );

    let info = GraphicPipelineInfo::default();

    Ok(Arc::new(GraphicPipeline::create(
        device,
        info,
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_fragment(frag.as_slice()),
        ],
    )?))
}

fn create_mesh_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 450 core

        layout(push_constant) uniform PushConstants {
            layout(offset = 0) mat4 model;
        } push_const;

        layout(set = 0, binding = 0) uniform Camera {
            mat4 view;
            mat4 projection;
        } camera;

        layout(location = 0) in vec3 position;
        layout(location = 1) in vec3 normal;

        out gl_PerVertex {
            vec4 gl_Position;
        };

        layout(location = 0) out vec4 world_position_out;
        layout(location = 1) out vec3 world_normal_out;

        void main()
        {
            world_normal_out = normalize((push_const.model * vec4(normal, 1.0)).xyz);
            world_position_out = push_const.model * vec4(position, 1.0);
            gl_Position = camera.projection * camera.view * world_position_out;
        }
        "#,
        vert
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core

        layout(constant_id = 0) const float BIAS = 0.15;

        layout(set = 0, binding = 1) uniform Light {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light;

        layout(set = 0, binding = 2) uniform samplerCube shadow_map;

        layout(location = 0) in vec4 world_position;
        layout(location = 1) in vec3 world_normal;

        layout(location = 0) out vec4 color_out;

        float upper_bound_shadow(vec2 moments, float scene_depth)
        {
            float p = step(scene_depth, moments.x + BIAS); // eliminates cubemap boundary thin line
            // 0 if moments.x < scene_depth; 1 if otherwise

            float variance = max(moments.y - moments.x * moments.x, 0.0001);
            // depth^2 - mean^2
            // ensure it as a denominator is not zero

            float dist = scene_depth - moments.x;
            float p_max = variance / (variance + dist * dist);

            return max(p, p_max);
        }

        float sample_shadow(samplerCube shadow_map, vec3 light, float scene_depth)
        {
            vec2 moments = texture(shadow_map, light).rg;
            // moments.r is mean, moments.g is depth^2

            return upper_bound_shadow(moments, scene_depth);
        }

        void main()
        {
            color_out = vec4(0.0, 0.0, 0.0, 1.0);

            vec3 light_dir = light.pos - world_position.xyz;
            float light_dist = length(light_dir);

            if (light_dist < light.range)
            {
                light_dir = normalize(light_dir);

                float shadow = sample_shadow(shadow_map, -light_dir, light_dist);
                float lambertian = max(0.0, dot(world_normal, light_dir));
                float attenuation = max(0.0, min(1.0, light_dist / light.range));
                attenuation = 1.0 - attenuation * attenuation;

                // Make shadows not fully dark
                shadow = max(0.1, shadow);

                color_out.rgb = vec3(attenuation * lambertian * shadow);
            }
        }
        "#,
        frag
    );

    let info = GraphicPipelineInfo::default();

    Ok(Arc::new(GraphicPipeline::create(
        device,
        info,
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_fragment(frag.as_slice()).specialization_info(SpecializationInfo::new(
                vec![vk::SpecializationMapEntry {
                    constant_id: 0,
                    offset: 0,
                    size: 4,
                }],
                bytes_of(&SHADOW_BIAS),
            )),
        ],
    )?))
}

fn create_shadow_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 450 core

        #define TAN_HALF_FOVY 1.0 // tan(45deg)
        #define ASPECT_RATIO 1.0
        #define LIGHT_FRUSTUM_NEAR 0.1

        struct ViewPositions {
            vec4 positions[6];
        };

        layout(push_constant) uniform PushConstants {
            layout(offset = 0) mat4 model;
        } push_const;

        layout(set = 0, binding = 0) uniform Light {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light;

        layout(location= 0) in vec3 position;

        layout(location = 0) out vec3 world_position_out;
        layout(location = 1) out uint layer_mask_out;
        layout(location = 2) out ViewPositions view_positions_out;

        uint get_layer_flag(vec4 view_pos, uint flag)
        {
            // if view_pos is in frustum, return flag
            // otherwise return 0

            uint res = 1;
            res *= uint(step(LIGHT_FRUSTUM_NEAR, -view_pos.z));
            res *= uint(step(-light.range, view_pos.z));

            float ymax = -TAN_HALF_FOVY * view_pos.z;
            float xmax = ymax * ASPECT_RATIO;
            res *= uint(step(-xmax, view_pos.x));
            res *= uint(step(view_pos.x, xmax));
            res *- uint(step(-ymax, view_pos.y));
            res *- uint(step(view_pos.y, ymax));

            return res * flag;
        }

        void main(void)
        {
            vec4 world_position = push_const.model * vec4(position, 1.0);
            world_position_out = world_position.xyz;

            // posx
            vec4 view0_pos = light.view0 * world_position;
            view0_pos.x *= -1.0;

            // negx
            vec4 view1_pos = vec4(-view0_pos.x, view0_pos.y, -view0_pos.z, 1.0);

            // posy
            vec4 view2_pos = vec4(-view0_pos.z, view0_pos.x, -view0_pos.y, 1.0);

            // negy
            vec4 view3_pos = vec4(-view0_pos.z, -view0_pos.x, view0_pos.y, 1.0);

            // posz
            vec4 view4_pos = vec4(-view0_pos.z, view0_pos.y, view0_pos.x, 1.0);

            // negz
            vec4 view5_pos = vec4(view0_pos.z, view0_pos.y, -view0_pos.x, 1.0);

            layer_mask_out = get_layer_flag(view0_pos, 0x01)
                           | get_layer_flag(view1_pos, 0x02)
                           | get_layer_flag(view2_pos, 0x04)
                           | get_layer_flag(view3_pos, 0x08)
                           | get_layer_flag(view4_pos, 0x16)
                           | get_layer_flag(view5_pos, 0x64);

            view_positions_out.positions[0] = view0_pos;
            view_positions_out.positions[1] = view1_pos;
            view_positions_out.positions[2] = view2_pos;
            view_positions_out.positions[3] = view3_pos;
            view_positions_out.positions[4] = view4_pos;
            view_positions_out.positions[5] = view5_pos;
        }
        "#,
        vert
    );
    let geom = inline_spirv!(
        r#"
        #version 450 core

        #define CLIP mat4(1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0)

        struct ViewPositions {
            vec4 positions[6];
        };

        layout(set = 0, binding = 0) uniform Light {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light;

        layout(triangles) in;
        layout(location = 0) in vec3 world_positions[];
        layout(location = 1) in uint layer_mask[];
        layout(location = 2) in ViewPositions view_positions[];

        out gl_PerVertex
        {
            vec4 gl_Position;
        };

        layout(triangle_strip, max_vertices = 18) out;
        layout(location = 0) out vec3 world_position_out;

        void emit(uint flag, int view_idx)
        {
            uint layer_flag = (layer_mask[0] | layer_mask[1] | layer_mask[2]) & flag;

            if (layer_flag > 0) {
                gl_Position = CLIP * light.projection * view_positions[0].positions[view_idx];
                world_position_out = world_positions[0];
                EmitVertex();

                gl_Position = CLIP * light.projection * view_positions[1].positions[view_idx];
                world_position_out = world_positions[1];
                EmitVertex();

                gl_Position = CLIP * light.projection * view_positions[2].positions[view_idx];
                world_position_out = world_positions[2];
                EmitVertex();

                EndPrimitive();
            }
        }

        void main()
        {
            gl_Layer = 0;
            emit(0x01, 0);

            gl_Layer = 1;
            emit(0x02, 1);

            gl_Layer = 2;
            emit(0x04, 2);

            gl_Layer = 3;
            emit(0x08, 3);

            gl_Layer = 4;
            emit(0x16, 4);

            gl_Layer = 5;
            emit(0x64, 5);
        }
        "#,
        geom
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core

        layout(set = 0, binding = 0) uniform Light {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light;

        layout(location = 0) in vec3 world_position;

        layout(location = 0) out vec4 color_out;

        void main()
        {
            float dist = distance(world_position.xyz, light.pos);
            color_out.x = dist;
            color_out.y = dist * dist;
        }
        "#,
        frag
    );

    let info = GraphicPipelineInfo::default();

    Ok(Arc::new(GraphicPipeline::create(
        device,
        info,
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_geometry(geom.as_slice()),
            Shader::new_fragment(frag.as_slice()),
        ],
    )?))
}

fn download_model_from_github(model_name: &str) -> anyhow::Result<PathBuf> {
    const REPO_URL: &str =
        "https://raw.githubusercontent.com/alecjacobson/common-3d-test-models/master/data/";

    let model_path = current_exe()?.parent().unwrap().join(model_name);
    let model_metadata = metadata(&model_path);

    if model_metadata.is_err() {
        info!("Downloading model from github");

        let data = reqwest::blocking::get(REPO_URL.to_owned() + model_name)?.bytes()?;
        write(&model_path, data)?;

        info!("Download complete");
    }

    Ok(model_path)
}

fn lease_uniform_buffer(
    pool: &mut impl Pool<BufferInfoBuilder, Buffer>,
    data: &impl NoUninit,
) -> Result<Lease<Buffer>, DriverError> {
    let data = bytes_of(data);
    let mut buf = pool.lease(BufferInfo::new_mappable(
        data.len() as _,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
    ))?;
    Buffer::copy_from_slice(&mut buf, 0, data);

    Ok(buf)
}

/// Returns vertices of a cube where the faces face inside
fn load_cube_data() -> [([f32; 6]); 36] {
    const N: f32 = -1f32;
    const P: f32 = 1f32;
    const Z: f32 = 0f32;

    const LEFT_BOTTOM_BACK: [f32; 3] = [N, N, P];
    const LEFT_BOTTOM_FRONT: [f32; 3] = [N, N, N];
    const LEFT_TOP_BACK: [f32; 3] = [N, P, P];
    const LEFT_TOP_FRONT: [f32; 3] = [N, P, N];
    const RIGHT_BOTTOM_BACK: [f32; 3] = [P, N, P];
    const RIGHT_BOTTOM_FRONT: [f32; 3] = [P, N, N];
    const RIGHT_TOP_BACK: [f32; 3] = [P, P, P];
    const RIGHT_TOP_FRONT: [f32; 3] = [P, P, N];

    const FORWARD: [f32; 3] = [Z, Z, P];
    const BACKWARD: [f32; 3] = [Z, Z, N];
    const LEFTWARD: [f32; 3] = [N, Z, Z];
    const RIGHTWARD: [f32; 3] = [P, Z, Z];
    const UPWARD: [f32; 3] = [Z, P, Z];
    const DOWNWARD: [f32; 3] = [Z, N, Z];

    const fn vertex(position: [f32; 3], normal: [f32; 3]) -> [f32; 6] {
        [
            position[0],
            position[1],
            position[2],
            normal[0],
            normal[1],
            normal[2],
        ]
    }

    [
        // Triangle 0
        vertex(LEFT_TOP_BACK, BACKWARD),
        vertex(LEFT_BOTTOM_BACK, BACKWARD),
        vertex(RIGHT_TOP_BACK, BACKWARD),
        // Triangle 1
        vertex(RIGHT_TOP_BACK, BACKWARD),
        vertex(LEFT_BOTTOM_BACK, BACKWARD),
        vertex(RIGHT_BOTTOM_BACK, BACKWARD),
        // // Triangle 2
        vertex(RIGHT_TOP_FRONT, FORWARD),
        vertex(RIGHT_BOTTOM_FRONT, FORWARD),
        vertex(LEFT_TOP_FRONT, FORWARD),
        // Triangle 3
        vertex(LEFT_TOP_FRONT, FORWARD),
        vertex(RIGHT_BOTTOM_FRONT, FORWARD),
        vertex(LEFT_BOTTOM_FRONT, FORWARD),
        // Triangle 4
        vertex(LEFT_TOP_FRONT, RIGHTWARD),
        vertex(LEFT_BOTTOM_FRONT, RIGHTWARD),
        vertex(LEFT_TOP_BACK, RIGHTWARD),
        // Triangle 5
        vertex(LEFT_TOP_BACK, RIGHTWARD),
        vertex(LEFT_BOTTOM_FRONT, RIGHTWARD),
        vertex(LEFT_BOTTOM_BACK, RIGHTWARD),
        // Triangle 6
        vertex(RIGHT_TOP_BACK, LEFTWARD),
        vertex(RIGHT_BOTTOM_BACK, LEFTWARD),
        vertex(RIGHT_TOP_FRONT, LEFTWARD),
        // Triangle 7
        vertex(RIGHT_TOP_FRONT, LEFTWARD),
        vertex(RIGHT_BOTTOM_BACK, LEFTWARD),
        vertex(RIGHT_BOTTOM_FRONT, LEFTWARD),
        // Triangle 8
        vertex(LEFT_BOTTOM_BACK, UPWARD),
        vertex(LEFT_BOTTOM_FRONT, UPWARD),
        vertex(RIGHT_BOTTOM_BACK, UPWARD),
        // Triangle 9
        vertex(RIGHT_BOTTOM_BACK, UPWARD),
        vertex(LEFT_BOTTOM_FRONT, UPWARD),
        vertex(RIGHT_BOTTOM_FRONT, UPWARD),
        // Triangle 10
        vertex(LEFT_TOP_FRONT, DOWNWARD),
        vertex(LEFT_TOP_BACK, DOWNWARD),
        vertex(RIGHT_TOP_FRONT, DOWNWARD),
        // Triangle 11
        vertex(RIGHT_TOP_FRONT, DOWNWARD),
        vertex(LEFT_TOP_BACK, DOWNWARD),
        vertex(RIGHT_TOP_BACK, DOWNWARD),
    ]
}

/// Loads a cube as indexed position and normal vertices
fn load_cube_mesh(device: &Arc<Device>) -> Result<Model, DriverError> {
    let vertices = load_cube_data();
    let indices = (0u32..vertices.len() as u32).collect::<Vec<_>>();

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        cast_slice(indices.as_slice()),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(vertices.as_slice()),
    )?);

    Ok(Model {
        index_buf,
        index_count: indices.len() as _,
        vertex_buf,
    })
}

/// Loads a cube as indexed position vertices
fn load_cube_shadow(device: &Arc<Device>) -> Result<Model, DriverError> {
    let vertices = load_cube_data()
        .iter()
        .map(|vertex| [vertex[0], vertex[1], vertex[2]])
        .collect::<Vec<_>>();
    let indices = (0u32..vertices.len() as u32).collect::<Vec<_>>();

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        cast_slice(indices.as_slice()),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(vertices.as_slice()),
    )?);

    Ok(Model {
        index_buf,
        index_count: indices.len() as _,
        vertex_buf,
    })
}

fn load_model<T>(
    device: &Arc<Device>,
    path: impl AsRef<Path>,
    face_fn: fn(a: Vec3, b: Vec3, c: Vec3) -> [T; 3],
) -> anyhow::Result<Model>
where
    T: Default + Pod,
{
    let (models, _) = load_obj(path.as_ref(), &GPU_LOAD_OPTIONS)?;
    let mut vertices =
        Vec::with_capacity(models.iter().map(|model| model.mesh.indices.len()).sum());

    // Calculate AABB
    let mut min = Vec3::ZERO;
    let mut max = Vec3::ZERO;
    for model in &models {
        for n in 0..model.mesh.positions.len() / 3 {
            let idx = 3 * n;
            let position = Vec3::from_slice(&model.mesh.positions[idx..idx + 3]);

            min = min.min(position);
            max = max.max(position);
        }
    }

    // Calculate a uniform scale which fits the model to a unit cube
    let scale = Vec3::splat(1.0 / (max - min).max_element());

    // Load the triangles using the face_fn closure to form vertices
    for model in models {
        for n in 0..model.mesh.indices.len() / 3 {
            let idx = 3 * n;
            let a_idx = 3 * model.mesh.indices[idx] as usize;
            let b_idx = 3 * model.mesh.indices[idx + 1] as usize;
            let c_idx = 3 * model.mesh.indices[idx + 2] as usize;
            let a = Vec3::from_slice(&model.mesh.positions[a_idx..a_idx + 3]) * scale;
            let b = Vec3::from_slice(&model.mesh.positions[b_idx..b_idx + 3]) * scale;
            let c = Vec3::from_slice(&model.mesh.positions[c_idx..c_idx + 3]) * scale;
            let face = face_fn(a, b, c);

            vertices.push(face[0]);
            vertices.push(face[1]);
            vertices.push(face[2]);
        }
    }

    // Re-index and de-dupe the model vertices using meshopt
    let indices = (0u32..vertices.len() as u32).collect::<Vec<_>>();
    let (vertex_count, remap) = generate_vertex_remap(&vertices, Some(&indices));
    let indices = remap_index_buffer(Some(&indices), vertex_count, &remap);
    let vertices = remap_vertex_buffer(&vertices, vertex_count, &remap);

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&indices),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&vertices),
    )?);

    Ok(Model {
        index_buf,
        index_count: indices.len() as _,
        vertex_buf,
    })
}

/// Loads an .obj model as indexed position and normal vertices
fn load_model_mesh(device: &Arc<Device>, path: impl AsRef<Path>) -> anyhow::Result<Model> {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Vertex {
        position: Vec3,
        normal: Vec3,
    }

    unsafe impl Pod for Vertex {}

    unsafe impl Zeroable for Vertex {}

    load_model(device, path, |a, b, c| {
        let u = b - a;
        let v = c - a;
        let normal = vec3(
            u.y * v.z - u.z * v.y,
            u.z * v.x - u.x * v.z,
            u.x * v.y - u.y * v.x,
        )
        .normalize();

        // Make faces CCW
        [
            Vertex {
                position: a,
                normal,
            },
            Vertex {
                position: c,
                normal,
            },
            Vertex {
                position: b,
                normal,
            },
        ]
    })
}

/// Loads an .obj model as indexed position vertices
fn load_model_shadow(device: &Arc<Device>, path: impl AsRef<Path>) -> anyhow::Result<Model> {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Vertex {
        position: Vec3,
    }

    unsafe impl Pod for Vertex {}

    unsafe impl Zeroable for Vertex {}

    load_model(device, path, |a, b, c| {
        // Make faces CCW
        [
            Vertex { position: a },
            Vertex { position: c },
            Vertex { position: b },
        ]
    })
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Blur {
    image_size: u32,
    radius: u32,
}

unsafe impl Pod for Blur {}

unsafe impl Zeroable for Blur {}

#[repr(C)]
#[derive(Clone, Copy)]
struct Camera {
    view: Mat4,
    projection: Mat4,
}

unsafe impl NoUninit for Camera {}

#[repr(C)]
#[derive(Clone, Copy)]
struct Light {
    position: Vec3,
    range: f32,
    view: Mat4,
    projection: Mat4,
}

unsafe impl NoUninit for Light {}

struct Model {
    index_buf: Arc<Buffer>,
    index_count: u32,
    vertex_buf: Arc<Buffer>,
}
