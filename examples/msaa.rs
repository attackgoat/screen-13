use {
    bytemuck::{bytes_of, cast_slice, NoUninit},
    glam::{Mat4, Vec3},
    inline_spirv::inline_spirv,
    screen_13::prelude::*,
    std::{mem::size_of, sync::Arc},
};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let mut keyboard = KeyBuf::default();
    let event_loop = EventLoop::new().debug(true).build()?;
    let samples = max_supported_samples(&event_loop);
    let mesh_pipeline = create_mesh_pipeline(&event_loop.device, samples)?;
    let cube_mesh = load_cube_mesh(&event_loop.device)?;
    let mut pool = HashPool::new(&event_loop.device);

    let mut angle = 0f32;

    event_loop.run(|frame| {
        update_keyboard(&mut keyboard, frame.events);

        angle += frame.dt * 0.1;
        let world_transform = Mat4::from_rotation_x(angle)
            * Mat4::from_rotation_y(angle * 0.61)
            * Mat4::from_rotation_z(angle * 0.22);

        let mut scene_uniform_buf = pool
            .lease(BufferInfo::new_mappable(
                size_of::<SceneUniformBuffer>() as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
            ))
            .unwrap();
        Buffer::copy_from_slice(
            &mut scene_uniform_buf,
            0,
            bytes_of(&SceneUniformBuffer {
                view: Mat4::look_at_lh(Vec3::Z * 4.0, Vec3::ZERO, Vec3::NEG_Y),
                projection: Mat4::perspective_lh(
                    45f32.to_radians(),
                    frame.render_aspect_ratio(),
                    0.1,
                    10.0,
                ),
                light_dir: Vec3::Y,
            }),
        );

        let msaa_color_image = frame.render_graph.bind_node(
            pool.lease(
                ImageInfo::new_2d(
                    frame.render_graph.node_info(frame.swapchain_image).fmt,
                    frame.width,
                    frame.height,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                )
                .sample_count(samples),
            )
            .unwrap(),
        );
        let msaa_depth_image = frame.render_graph.bind_node(
            pool.lease(
                ImageInfo::new_2d(
                    vk::Format::D32_SFLOAT,
                    frame.width,
                    frame.height,
                    vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                )
                .sample_count(samples),
            )
            .unwrap(),
        );
        let depth_image = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d(
                vk::Format::D32_SFLOAT,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            ))
            .unwrap(),
        );

        let cube_vertex_buf = frame.render_graph.bind_node(&cube_mesh.vertex_buf);
        let scene_uniform_buf = frame.render_graph.bind_node(scene_uniform_buf);

        let mut pass = frame
            .render_graph
            .begin_pass("cube")
            .bind_pipeline(&mesh_pipeline)
            .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
            .access_node(cube_vertex_buf, AccessType::VertexBuffer)
            .access_descriptor(0, scene_uniform_buf, AccessType::AnyShaderReadUniformBuffer);

        if !keyboard.any_held() {
            // Render in multisample mode
            pass = pass
                .clear_color_value(0, msaa_color_image, [1.0, 1.0, 1.0, 1.0])
                .resolve_color(0, 1, frame.swapchain_image)
                .clear_depth_stencil(msaa_depth_image);
        } else {
            // Render in non-multisample mode
            pass = pass
                .clear_color_value(0, frame.swapchain_image, [1.0, 1.0, 1.0, 1.0])
                .store_color(0, frame.swapchain_image)
                .clear_depth_stencil(depth_image);
        }

        pass.record_subpass(move |subpass, _| {
            subpass
                .bind_vertex_buffer(cube_vertex_buf)
                .push_constants(bytes_of(&world_transform))
                .draw(cube_mesh.vertex_count, 1, 0, 0);
        });
    })?;

    Ok(())
}

fn max_supported_samples(event_loop: &EventLoop) -> SampleCount {
    let limit = event_loop
        .device
        .physical_device
        .props
        .limits
        .sampled_image_color_sample_counts;
    if limit.contains(vk::SampleCountFlags::TYPE_64) {
        SampleCount::X64
    } else if limit.contains(vk::SampleCountFlags::TYPE_32) {
        SampleCount::X32
    } else if limit.contains(vk::SampleCountFlags::TYPE_16) {
        SampleCount::X16
    } else if limit.contains(vk::SampleCountFlags::TYPE_8) {
        SampleCount::X8
    } else if limit.contains(vk::SampleCountFlags::TYPE_4) {
        SampleCount::X4
    } else if limit.contains(vk::SampleCountFlags::TYPE_2) {
        SampleCount::X2
    } else {
        warn!("MSAA not supported");

        SampleCount::X1
    }
}

/// Returns vertices of a colored cube
fn load_cube_data() -> [CubeVertex; 36] {
    type Position = [f32; 3];
    type Normal = [f32; 3];
    type Color = [f32; 3];

    const N: f32 = -1f32;
    const P: f32 = 1f32;
    const Z: f32 = 0f32;

    const LEFT_BOTTOM_BACK: Position = [N, N, P];
    const LEFT_BOTTOM_FRONT: Position = [N, N, N];
    const LEFT_TOP_BACK: Position = [N, P, P];
    const LEFT_TOP_FRONT: Position = [N, P, N];
    const RIGHT_BOTTOM_BACK: Position = [P, N, P];
    const RIGHT_BOTTOM_FRONT: Position = [P, N, N];
    const RIGHT_TOP_BACK: Position = [P, P, P];
    const RIGHT_TOP_FRONT: Position = [P, P, N];

    const FORWARD: Normal = [Z, Z, P];
    const BACKWARD: Normal = [Z, Z, N];
    const LEFTWARD: Normal = [N, Z, Z];
    const RIGHTWARD: Normal = [P, Z, Z];
    const UPWARD: Normal = [Z, P, Z];
    const DOWNWARD: Normal = [Z, N, Z];

    const RED: Color = [1.0, 0.0, 0.0];
    const GREEN: Color = [0.0, 1.0, 0.0];
    const BLUE: Color = [0.0, 0.0, 1.0];
    const YELLOW: Color = [1.0, 1.0, 0.0];
    const CYAN: Color = [0.0, 1.0, 1.0];
    const MAGENTA: Color = [1.0, 0.0, 1.0];

    const fn vertex(position: Position, normal: Normal, color: Color) -> CubeVertex {
        [position, normal, color]
    }

    [
        // Triangle 0
        vertex(LEFT_TOP_BACK, FORWARD, RED),
        vertex(LEFT_BOTTOM_BACK, FORWARD, RED),
        vertex(RIGHT_TOP_BACK, FORWARD, RED),
        // Triangle 1
        vertex(RIGHT_TOP_BACK, FORWARD, RED),
        vertex(LEFT_BOTTOM_BACK, FORWARD, RED),
        vertex(RIGHT_BOTTOM_BACK, FORWARD, RED),
        // // Triangle 2
        vertex(RIGHT_TOP_FRONT, BACKWARD, GREEN),
        vertex(RIGHT_BOTTOM_FRONT, BACKWARD, GREEN),
        vertex(LEFT_TOP_FRONT, BACKWARD, GREEN),
        // Triangle 3
        vertex(LEFT_TOP_FRONT, BACKWARD, GREEN),
        vertex(RIGHT_BOTTOM_FRONT, BACKWARD, GREEN),
        vertex(LEFT_BOTTOM_FRONT, BACKWARD, GREEN),
        // Triangle 4
        vertex(LEFT_TOP_FRONT, LEFTWARD, BLUE),
        vertex(LEFT_BOTTOM_FRONT, LEFTWARD, BLUE),
        vertex(LEFT_TOP_BACK, LEFTWARD, BLUE),
        // Triangle 5
        vertex(LEFT_TOP_BACK, LEFTWARD, BLUE),
        vertex(LEFT_BOTTOM_FRONT, LEFTWARD, BLUE),
        vertex(LEFT_BOTTOM_BACK, LEFTWARD, BLUE),
        // Triangle 6
        vertex(RIGHT_TOP_BACK, RIGHTWARD, YELLOW),
        vertex(RIGHT_BOTTOM_BACK, RIGHTWARD, YELLOW),
        vertex(RIGHT_TOP_FRONT, RIGHTWARD, YELLOW),
        // Triangle 7
        vertex(RIGHT_TOP_FRONT, RIGHTWARD, YELLOW),
        vertex(RIGHT_BOTTOM_BACK, RIGHTWARD, YELLOW),
        vertex(RIGHT_BOTTOM_FRONT, RIGHTWARD, YELLOW),
        // Triangle 8
        vertex(LEFT_BOTTOM_BACK, DOWNWARD, CYAN),
        vertex(LEFT_BOTTOM_FRONT, DOWNWARD, CYAN),
        vertex(RIGHT_BOTTOM_BACK, DOWNWARD, CYAN),
        // Triangle 9
        vertex(RIGHT_BOTTOM_BACK, DOWNWARD, CYAN),
        vertex(LEFT_BOTTOM_FRONT, DOWNWARD, CYAN),
        vertex(RIGHT_BOTTOM_FRONT, DOWNWARD, CYAN),
        // Triangle 10
        vertex(LEFT_TOP_FRONT, UPWARD, MAGENTA),
        vertex(LEFT_TOP_BACK, UPWARD, MAGENTA),
        vertex(RIGHT_TOP_FRONT, UPWARD, MAGENTA),
        // Triangle 11
        vertex(RIGHT_TOP_FRONT, UPWARD, MAGENTA),
        vertex(LEFT_TOP_BACK, UPWARD, MAGENTA),
        vertex(RIGHT_TOP_BACK, UPWARD, MAGENTA),
    ]
}

/// Loads a cube as unindexed position, normal and color vertices
fn load_cube_mesh(device: &Arc<Device>) -> Result<Model, DriverError> {
    let vertices = load_cube_data();

    let vertex_buf = Arc::new(Buffer::create_from_slice(
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(vertices.as_slice()),
    )?);

    Ok(Model {
        vertex_buf,
        vertex_count: vertices.len() as _,
    })
}

fn create_mesh_pipeline(
    device: &Arc<Device>,
    samples: SampleCount,
) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vert = inline_spirv!(
        r#"
        #version 460 core

        layout(push_constant) uniform PushConstants {
            mat4 world;
        } push_const;

        layout(set = 0, binding = 0) uniform Scene {
            mat4 view;
            mat4 projection;
            vec3 light_dir;
        } scene;

        layout(location = 0) in vec3 position;
        layout(location = 1) in vec3 normal;
        layout(location = 2) in vec3 color;

        layout(location = 0) out vec3 normal_out;
        layout(location = 1) out vec3 color_out;

        void main() {
            gl_Position = scene.projection * scene.view * push_const.world * vec4(position, 1.0);

            normal_out = (push_const.world * vec4(normal, 1.0)).xyz;
            color_out = color;
        }
        "#,
        vert
    );
    let frag = inline_spirv!(
        r#"
        #version 460 core

        layout(set = 0, binding = 0) uniform Scene {
            mat4 view;
            mat4 projection;
            vec3 light_dir;
        } scene;

        layout(location = 0) in vec3 normal;
        layout(location = 1) in vec3 color;

        layout(location = 0) out vec4 color_out;

        void main() {
            float lambertian = max(0.25, dot(normal, scene.light_dir));

            color_out = vec4(color * lambertian, 1.0);
        }
        "#,
        frag
    );

    let info = GraphicPipelineInfo::new().samples(samples);

    Ok(Arc::new(GraphicPipeline::create(
        device,
        info,
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_fragment(frag.as_slice()),
        ],
    )?))
}

type CubeVertex = [[f32; 3]; 3];

struct Model {
    vertex_buf: Arc<Buffer>,
    vertex_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SceneUniformBuffer {
    view: Mat4,
    projection: Mat4,
    light_dir: Vec3,
}

unsafe impl NoUninit for SceneUniformBuffer {}
