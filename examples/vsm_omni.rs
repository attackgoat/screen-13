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

const CUBEMAP_SIZE: u32 = 512;
const RANGE: f32 = 100.0f32;
const RENDER_DEBUG: bool = false;
const SCALE: f32 = 10.0f32;

/// Adapted from https://github.com/sydneyzh/variance_shadow_mapping_vk
///
/// This example does an HTTPS GET to acquire model data!
fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let shadow_cube_faces_info = ImageInfo::new_2d_array(
        vk::Format::R32G32_SFLOAT,
        CUBEMAP_SIZE,
        CUBEMAP_SIZE,
        6,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
    )
    .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
    .build();

    // Change the source of this function if you want different models
    let model_path = download_model_from_github()?;

    let event_loop = EventLoop::new().debug(true).build()?;

    // Load all the immutable graphics data we will need
    let model_mesh = load_model_mesh(&event_loop.device, &model_path)?;
    let model_shadow = load_model_shadow(&event_loop.device, &model_path)?;
    let cube = load_cube(&event_loop.device)?;
    let debug_pipeline = create_debug_pipeline(&event_loop.device)?;
    let blur_x_pipeline = create_blur_x_pipeline(&event_loop.device)?;
    let blur_y_pipeline = create_blur_y_pipeline(&event_loop.device)?;
    let mesh_pipeline = create_mesh_pipeline(&event_loop.device)?;
    let shadow_pipeline = create_shadow_pipeline(&event_loop.device)?;

    // A pool will be used for per-frame resources
    let mut pool = LazyPool::new(&event_loop.device);

    let mut elapsed = 0f32;

    event_loop.run(|frame| {
        elapsed += frame.dt;

        // Calculate values for and fill some plain-old-data structs we will bind as UBO's
        let light_data = {
            let fov_y = 90f32.to_radians();
            let radius = SCALE * 0.95f32;
            let speed = elapsed / 2.0;
            let position = vec3(speed.cos() * radius, SCALE * 0.25, speed.sin() * radius);

            LightUniformBuffer {
                position,
                range: RANGE,
                view: Mat4::look_at_lh(position, position - Vec3::Z, Vec3::Y),
                projection: Mat4::perspective_lh(fov_y, 1.0, 0.01, SCALE),
            }
        };
        let mesh_data = {
            let aspect_ratio = frame.width as f32 / frame.height as f32;
            let fov_y = 45f32.to_radians();
            let projection = Mat4::perspective_lh(fov_y, aspect_ratio, 0.01, SCALE);

            let position = vec3(0.0, 0.0, SCALE * 2.5);
            let view = Mat4::look_at_lh(position, position - Vec3::Z, Vec3::Y);

            let model = Mat4::from_scale_rotation_translation(
                Vec3::splat(SCALE),
                Quat::from_rotation_y(180f32.to_radians())
                    * Quat::from_rotation_x(90f32.to_radians()),
                Vec3::ZERO,
            );

            let normal = (view * model).transpose().inverse();

            MeshUniformBuffer {
                view,
                model,
                normal,
                projection,
                clip: Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.5),
                    Quat::IDENTITY,
                    vec3(-0.0, -0.0, 0.0),
                ),
            }
        };

        // Bind resources to the render graph of the current frame
        let cube_index_buf = frame.render_graph.bind_node(&cube.index_buf);
        let cube_vertex_buf = frame.render_graph.bind_node(&cube.vertex_buf);
        let model_mesh_index_buf = frame.render_graph.bind_node(&model_mesh.index_buf);
        let model_mesh_vertex_buf = frame.render_graph.bind_node(&model_mesh.vertex_buf);
        let model_shadow_index_buf = frame.render_graph.bind_node(&model_shadow.index_buf);
        let model_shadow_vertex_buf = frame.render_graph.bind_node(&model_shadow.vertex_buf);
        let light_uniform_buf = frame
            .render_graph
            .bind_node(lease_uniform_buffer(&mut pool, &light_data).unwrap());
        let mesh_uniform_buf = frame
            .render_graph
            .bind_node(lease_uniform_buffer(&mut pool, &mesh_data).unwrap());
        let shadow_cube_faces_image = frame
            .render_graph
            .bind_node(pool.lease(shadow_cube_faces_info).unwrap());
        let depth_image = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d(
                vk::Format::D32_SFLOAT,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            ))
            .unwrap(),
        );

        if RENDER_DEBUG {
            frame
                .render_graph
                .begin_pass("DEBUG")
                .bind_pipeline(&debug_pipeline)
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(0, mesh_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_descriptor(1, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_node(model_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(model_mesh_vertex_buf, AccessType::VertexBuffer)
                .access_node(cube_index_buf, AccessType::IndexBuffer)
                .access_node(cube_vertex_buf, AccessType::VertexBuffer)
                .clear_color(0, frame.swapchain_image)
                .store_color(0, frame.swapchain_image)
                .clear_depth_stencil(depth_image)
                .store_depth_stencil(depth_image)
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_mesh_vertex_buf)
                        .draw_indexed(model_mesh.index_count, 1, 0, 0, 0)
                        .bind_index_buffer(cube_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(cube_vertex_buf)
                        .draw_indexed(cube.index_count, 1, 0, 0, 0);
                });
        } else {
            // Render the omni light point of view into the six-layer image we leased above
            frame
                .render_graph
                .begin_pass("Shadow")
                .bind_pipeline(&shadow_pipeline)
                // .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(0, mesh_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_descriptor(1, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_node(model_shadow_index_buf, AccessType::IndexBuffer)
                .access_node(model_shadow_vertex_buf, AccessType::VertexBuffer)
                .clear_color_value(0, shadow_cube_faces_image, [0.0f32, 0.0, 0.0, 0.0])
                .store_color(0, shadow_cube_faces_image)
                // .clear_depth_stencil(depth_image)
                // .store_depth_stencil(depth_image)
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_shadow_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_shadow_vertex_buf)
                        .draw_indexed(model_shadow.index_count, 1, 0, 0, 0);
                });

            // Render the scene directly to the swapchain using the shadow map from the above pass
            frame
                .render_graph
                .begin_pass("Mesh objects")
                .bind_pipeline(&mesh_pipeline)
                .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
                .access_descriptor(0, mesh_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .access_descriptor(1, light_uniform_buf, AccessType::AnyShaderReadUniformBuffer)
                .read_descriptor_as(
                    2,
                    shadow_cube_faces_image,
                    shadow_cube_faces_info
                        .default_view_info()
                        .with_ty(ImageType::Cube),
                )
                .access_node(model_mesh_index_buf, AccessType::IndexBuffer)
                .access_node(model_mesh_vertex_buf, AccessType::VertexBuffer)
                .access_node(cube_index_buf, AccessType::IndexBuffer)
                .access_node(cube_vertex_buf, AccessType::VertexBuffer)
                .clear_color(0, frame.swapchain_image)
                .store_color(0, frame.swapchain_image)
                .clear_depth_stencil(depth_image)
                .store_depth_stencil(depth_image)
                .record_subpass(move |subpass, _| {
                    subpass
                        .bind_index_buffer(model_mesh_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(model_mesh_vertex_buf)
                        .draw_indexed(model_mesh.index_count, 1, 0, 0, 0)
                        .bind_index_buffer(cube_index_buf, vk::IndexType::UINT32)
                        .bind_vertex_buffer(cube_vertex_buf)
                        .draw_indexed(cube.index_count, 1, 0, 0, 0);
                });
        }
    })?;

    Ok(())
}

fn create_blur_x_pipeline(device: &Arc<Device>) -> Result<Arc<ComputePipeline>, DriverError> {
    let comp = inline_spirv!(
        r#"
        #version 450 core

        void main() {
           
        }
        "#,
        comp
    );

    Ok(Arc::new(ComputePipeline::create(device, comp.as_slice())?))
}

fn create_blur_y_pipeline(device: &Arc<Device>) -> Result<Arc<ComputePipeline>, DriverError> {
    let comp = inline_spirv!(
        r#"
        #version 450 core

        void main() {
           
        }
        "#,
        comp
    );

    Ok(Arc::new(ComputePipeline::create(device, comp.as_slice())?))
}

fn create_debug_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 450 core

        layout(set = 0, binding = 0) uniform UBO
        {
            mat4 view;
            mat4 model;
            mat4 normal;
            mat4 projection;
            mat4 clip;
        } ubo_in;

        layout (location = 0) in vec3 position_in;
        layout (location = 1) in vec3 normal_in;

        layout (location = 0) out vec3 position_out;
        layout (location = 1) out vec3 normal_out;

        void main() {
            position_out = (ubo_in.model * vec4(position_in, 1)).xyz;
            normal_out = normalize((ubo_in.normal * vec4(normal_in, 1)).xyz);
            gl_Position = ubo_in.clip * ubo_in.projection * ubo_in.view * vec4(position_out, 1);
        }
        "#,
        vert
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core

        layout(set = 0, binding = 1) uniform PLight_info
        {
            vec3 position;
            float range;
            mat4 view0;
            mat4 projection;
        } light_info_in;

        layout (location = 0) in vec3 position_in;
        layout (location = 1) in vec3 normal_in;

        layout (location = 0) out vec4 color_out;

        void main() {
            vec3 light_dir = normalize(vec3(light_info_in.position));
            float intensity = max(dot(normal_in, -light_dir), 0.01);
            color_out = vec4(intensity.xxx, 1);
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

        layout (location= 0) in vec3 pos_in;
        layout (location= 1) in vec3 normal_in;

        layout(set = 0, binding = 0) uniform UBO
        {
            mat4 view;
            mat4 model;
            mat4 normal;
            mat4 projection;
            mat4 clip;
        } ubo_in;

        out gl_PerVertex
        {
            vec4 gl_Position;
        };

        layout (location = 0) out vec4 world_pos_out;
        layout (location = 1) out vec3 world_normal_out;

        void main() {
            world_normal_out = (ubo_in.normal * vec4(normal_in, 1.f)).xyz;
            world_pos_out = ubo_in.model * vec4(pos_in, 1.f);
            gl_Position = ubo_in.clip * ubo_in.projection * ubo_in.view * world_pos_out;
        }
        "#,
        vert
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core
        #define BIAS 0.15f

        layout(set = 0, binding = 0) uniform UBO
        {
            mat4 view;
            mat4 model;
            mat4 normal;
            mat4 projection;
            mat4 clip;
        } ubo_in;

        layout(set = 0, binding = 1) uniform LightInfo
        {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light_info_in;

        layout(set = 0, binding = 2) uniform samplerCube shadow_map;

        layout(location = 0) in vec4 world_pos_in;
        layout(location = 1) in vec3 world_normal_in;
        layout(location = 0) out vec4 frag_color;

        float upper_bound_shadow(vec2 moments, float scene_depth)
        {
            float p = step(scene_depth, moments.x + BIAS); // eliminates cubemap boundary thin line
            // 0 if moments.x < scene_depth; 1 if otherwise

            float variance = max(moments.y - moments.x * moments.x, 0.0001);
            // depth^2 - mean^2
            // ensure it as a denominator is not zero

            float d = scene_depth - moments.x;

            float p_max = variance / (variance + d * d);
            // from the theorem

            return max(p, p_max);
        }

        float sample_shadow(samplerCube shadow_map, vec3 l, float scene_depth)
        {
            vec2 moments = texture(shadow_map, l).xy;
            // moments.x is mean, moments.y is depth^2

            return upper_bound_shadow(moments, scene_depth);
        }

        void main() {
            frag_color = vec4(0.0, 0.0, 0.0, 1.0);
            vec3 l = world_pos_in.xyz - light_info_in.pos;
            float d = length(l);
            l = normalize(l);
            if (d < light_info_in.range) {
                float lambertian = max(0.0, dot(world_normal_in, -l));
                lambertian = 1.0;
                float atten = max(0.0, min(1.0, d / light_info_in.range));
                atten = 1.f - atten * atten;
                float shadow = sample_shadow(shadow_map, l, d);
                frag_color.xyz += vec3(atten * lambertian * shadow);
           }

        //    frag_color = vec4(1.0, 0.0, 1.0, 1.0);
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

fn create_shadow_pipeline(device: &Arc<Device>) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 450 core

        #define TAN_HALF_FOVY 1.0f // tan(45deg)
        #define ASP 1.0f
        #define LIGHT_FRUSTUM_NEAR 0.01f
        
        layout (location= 0) in vec3 pos_in;

        layout(set = 0, binding = 0) uniform UBO
        {
            mat4 view;
            mat4 model;
            mat4 normal;
            mat4 projection;
            mat4 clip;
        } ubo_in;

        layout(set = 0, binding = 1) uniform LightInfo
        {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light_info_in;

        layout (location = 0) out vec3 world_pos_out;
        layout (location = 1) out uint layer_mask_out;

        struct View_positions
        {
            vec4 positions[6];
        };
        layout (location = 2) out View_positions view_positions_out;

        uint get_layer_flag(vec4 view_pos, uint flag)
        {
            // if view_pos is in frustum, return flag
            // otherwise return 0

            uint res = 1;
            res *= uint(step(LIGHT_FRUSTUM_NEAR, -view_pos.z));
            res *= uint(step(-light_info_in.range, view_pos.z));

            float ymax = - TAN_HALF_FOVY * view_pos.z;
            float xmax = ymax * ASP;
            res *= uint(step(-xmax, view_pos.x));
            res *= uint(step(view_pos.x, xmax));
            res *- uint(step(-ymax, view_pos.y));
            res *- uint(step(view_pos.y, ymax));

            return res * flag;
        }

        void main(void)
        {
            vec4 world_pos = ubo_in.model * vec4(pos_in, 1.0f);
            world_pos_out = world_pos.xyz;

            // posx
            vec4 view0_pos = light_info_in.view0 * world_pos;
            view0_pos.x *= -1.0f;

            // negx
            vec4 view1_pos = vec4(-view0_pos.x, view0_pos.y, -view0_pos.z, 1.0f);

            // posy
            vec4 view2_pos = vec4(-view0_pos.z, view0_pos.x, -view0_pos.y, 1.0f);

            // negy
            vec4 view3_pos = vec4(-view0_pos.z, -view0_pos.x, view0_pos.y, 1.0f);

            // posz
            vec4 view4_pos = vec4(-view0_pos.z, view0_pos.y, view0_pos.x, 1.0f);

            // negz
            vec4 view5_pos = vec4(view0_pos.z, view0_pos.y, -view0_pos.x, 1.0f);

            layer_mask_out = get_layer_flag(view0_pos, 1)
                           | get_layer_flag(view1_pos, 2)
                           | get_layer_flag(view2_pos, 4)
                           | get_layer_flag(view3_pos, 8)
                           | get_layer_flag(view4_pos, 16)
                           | get_layer_flag(view5_pos, 32);

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

        layout(triangles) in;

        layout(set = 0, binding = 0) uniform UBO
        {
            mat4 view;
            mat4 model;
            mat4 normal;
            mat4 projection;
            mat4 clip;
        } ubo_in;

        layout(set = 0, binding = 1) uniform Light_info
        {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light_info_in;

        layout(location = 0) in vec3 world_pos_in[];
        layout(location = 1) in uint layer_mask_in[];

        struct View_positions
        {
            vec4 positions[6];
        };
        layout (location = 2) in View_positions view_positions_in[];

        layout(triangle_strip, max_vertices = 18) out;
        out gl_PerVertex
        {
            vec4 gl_Position;
        };
        layout(location = 0) out vec3 world_pos_out;

        void emit(uint flag, int view_idx)
        {
            vec4 pos0 = ubo_in.clip * light_info_in.projection * view_positions_in[0].positions[view_idx];
            vec4 pos1 = ubo_in.clip * light_info_in.projection * view_positions_in[1].positions[view_idx];
            vec4 pos2 = ubo_in.clip * light_info_in.projection * view_positions_in[2].positions[view_idx];

            if (flag > 0) {
                gl_Position = pos0;
                world_pos_out = world_pos_in[0];
                EmitVertex();

                gl_Position = pos1;
                world_pos_out = world_pos_in[1];
                EmitVertex();

                gl_Position = pos2;
                world_pos_out = world_pos_in[2];
                EmitVertex();

                EndPrimitive();
            }
        }

        void main()
        {
            uint layer_flag_0 = (layer_mask_in[0] & 1)
                              | (layer_mask_in[1] & 1)
                              | (layer_mask_in[2] & 1);

            uint layer_flag_1 = (layer_mask_in[0] & 2)
                              | (layer_mask_in[1] & 2)
                              | (layer_mask_in[2] & 2);

            uint layer_flag_2 = (layer_mask_in[0] & 4)
                              | (layer_mask_in[1] & 4)
                              | (layer_mask_in[2] & 4);

            uint layer_flag_3 = (layer_mask_in[0] & 8)
                              | (layer_mask_in[1] & 8)
                              | (layer_mask_in[2] & 8);

            uint layer_flag_4 = (layer_mask_in[0] & 16)
                              | (layer_mask_in[1] & 16)
                              | (layer_mask_in[2] & 16);

            uint layer_flag_5 = (layer_mask_in[0] & 32)
                              | (layer_mask_in[1] & 32)
                              | (layer_mask_in[2] & 32);

            gl_Layer = 0;
            emit(layer_flag_0, 0);

            gl_Layer = 1;
            emit(layer_flag_1, 1);

            gl_Layer = 2;
            emit(layer_flag_2, 2);

            gl_Layer = 3;
            emit(layer_flag_3, 3);

            gl_Layer = 4;
            emit(layer_flag_4, 4);

            gl_Layer = 5;
            emit(layer_flag_5, 5);
        }
        "#,
        geom
    );
    let frag = inline_spirv!(
        r#"
        #version 450 core

        layout(set = 0, binding = 1) uniform Light_info
        {
            vec3 pos;
            float range;
            mat4 view0;
            mat4 projection;
        } light_info_in;

        layout(location = 0) in vec3 world_pos_in;

        layout (location = 0) out vec4 frag_color;

        void main() {
            float d = distance(world_pos_in.xyz, light_info_in.pos);
            frag_color.x = d;
            frag_color.y = d * d;
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

fn download_model_from_github() -> anyhow::Result<PathBuf> {
    const MODEL_NAME: &str = "nefertiti.obj";
    const REPO_URL: &str = "https://github.com/alecjacobson/common-3d-test-models/raw/master/data/";

    let model_path = current_exe()?.parent().unwrap().join(MODEL_NAME);
    let model_metadata = metadata(&model_path);

    if model_metadata.is_err() {
        info!("Downloading model from github");

        let data = reqwest::blocking::get(REPO_URL.to_owned() + MODEL_NAME)?.bytes()?;
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

/// Loads a cube where the faces face inside
fn load_cube(device: &Arc<Device>) -> Result<Model, DriverError> {
    // The index buffer here isn't optimal and *that's okay* its legible

    const N: f32 = -1f32;
    const P: f32 = 1f32;
    const Z: f32 = 0f32;

    const TOP_LEFT_BACK: [f32; 3] = [N, P, N];
    const TOP_LEFT_FRONT: [f32; 3] = [N, P, P];
    const TOP_RIGHT_BACK: [f32; 3] = [P, P, N];
    const TOP_RIGHT_FRONT: [f32; 3] = [P, P, P];
    const BOTTOM_LEFT_BACK: [f32; 3] = [N, N, N];
    const BOTTOM_LEFT_FRONT: [f32; 3] = [N, N, P];
    const BOTTOM_RIGHT_BACK: [f32; 3] = [P, N, N];
    const BOTTOM_RIGHT_FRONT: [f32; 3] = [P, N, P];

    const FORWARD: [f32; 3] = [Z, Z, P];
    const BACKWARD: [f32; 3] = [Z, Z, N];
    const LEFT: [f32; 3] = [P, Z, Z];
    const RIGHT: [f32; 3] = [N, Z, Z];
    const UP: [f32; 3] = [Z, P, Z];
    const DOWN: [f32; 3] = [Z, N, Z];

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

    let index_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        cast_slice(
            [
                0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
            ]
            .as_slice(),
        ),
    )?);
    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(
            [
                // Triangle 0
                vertex(TOP_LEFT_BACK, FORWARD),
                vertex(BOTTOM_LEFT_BACK, FORWARD),
                vertex(TOP_RIGHT_BACK, FORWARD),
                // Triangle 1
                vertex(TOP_RIGHT_BACK, FORWARD),
                vertex(BOTTOM_LEFT_BACK, FORWARD),
                vertex(BOTTOM_RIGHT_BACK, FORWARD),
                // // Triangle 2
                vertex(TOP_RIGHT_FRONT, BACKWARD),
                vertex(BOTTOM_RIGHT_FRONT, BACKWARD),
                vertex(TOP_LEFT_FRONT, BACKWARD),
                // Triangle 3
                vertex(TOP_LEFT_FRONT, BACKWARD),
                vertex(BOTTOM_RIGHT_FRONT, BACKWARD),
                vertex(BOTTOM_LEFT_FRONT, BACKWARD),
                // Triangle 4
                vertex(TOP_LEFT_FRONT, LEFT),
                vertex(BOTTOM_LEFT_FRONT, LEFT),
                vertex(TOP_LEFT_BACK, LEFT),
                // Triangle 5
                vertex(TOP_LEFT_BACK, LEFT),
                vertex(BOTTOM_LEFT_FRONT, LEFT),
                vertex(BOTTOM_LEFT_BACK, LEFT),
                // Triangle 6
                vertex(TOP_RIGHT_BACK, RIGHT),
                vertex(BOTTOM_RIGHT_BACK, RIGHT),
                vertex(TOP_RIGHT_FRONT, RIGHT),
                // Triangle 7
                vertex(TOP_RIGHT_FRONT, RIGHT),
                vertex(BOTTOM_RIGHT_BACK, RIGHT),
                vertex(BOTTOM_RIGHT_FRONT, RIGHT),
                // Triangle 8
                vertex(BOTTOM_LEFT_BACK, UP),
                vertex(BOTTOM_LEFT_FRONT, UP),
                vertex(BOTTOM_RIGHT_BACK, UP),
                // Triangle 9
                vertex(BOTTOM_RIGHT_BACK, UP),
                vertex(BOTTOM_LEFT_FRONT, UP),
                vertex(BOTTOM_RIGHT_FRONT, UP),
                // Triangle 10
                vertex(TOP_LEFT_FRONT, DOWN),
                vertex(TOP_LEFT_BACK, DOWN),
                vertex(TOP_RIGHT_FRONT, DOWN),
                // Triangle 11
                vertex(TOP_RIGHT_FRONT, DOWN),
                vertex(TOP_LEFT_BACK, DOWN),
                vertex(TOP_RIGHT_BACK, DOWN),
            ]
            .as_slice(),
        ),
    )?);

    Ok(Model {
        index_buf,
        index_count: 36,
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
        );

        [
            Vertex {
                position: a,
                normal,
            },
            Vertex {
                position: b,
                normal,
            },
            Vertex {
                position: c,
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
        [
            Vertex { position: a },
            Vertex { position: b },
            Vertex { position: c },
        ]
    })
}

struct Model {
    index_buf: Arc<Buffer>,
    index_count: u32,
    vertex_buf: Arc<Buffer>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LightUniformBuffer {
    position: Vec3,
    range: f32,
    view: Mat4,
    projection: Mat4,
}

unsafe impl NoUninit for LightUniformBuffer {}

#[repr(C)]
#[derive(Clone, Copy)]
struct MeshUniformBuffer {
    view: Mat4,
    model: Mat4,
    normal: Mat4,
    projection: Mat4,
    clip: Mat4,
}

unsafe impl NoUninit for MeshUniformBuffer {}
