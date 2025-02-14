mod profile_with_puffin;

use {
    bytemuck::{bytes_of, cast_slice, NoUninit},
    clap::Parser,
    glam::{Mat4, Vec3},
    inline_spirv::inline_spirv,
    log::warn,
    screen_13::prelude::*,
    screen_13_window::WindowBuilder,
    std::{mem::size_of, sync::Arc},
    winit::{event::Event, keyboard::KeyCode},
    winit_input_helper::WinitInputHelper,
};

type CubeVertex = [[f32; 3]; 3];

const WHITE: ClearColorValue = ClearColorValue([1.0, 1.0, 1.0, 1.0]);

/// Draws a spinning cube with high-contrast edges; hold any key to display the cube in non-MSAA
/// mode.
///
/// NOTE: The effect may be hard to see on high-DPI displays.
fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    profile_with_puffin::init();

    let mut input = WinitInputHelper::default();
    let args = Args::parse();
    let window = WindowBuilder::default().debug(args.debug).build()?;
    let depth_format = best_depth_format(&window.device);
    let sample_count = max_supported_sample_count(&window.device);
    let mesh_msaa_pipeline = create_mesh_pipeline(&window.device, sample_count)?;
    let mesh_noaa_pipeline = create_mesh_pipeline(&window.device, SampleCount::Type1)?;
    let cube_mesh = load_cube_mesh(&window.device)?;
    let mut pool = FifoPool::new(&window.device);

    let mut angle = 0f32;

    window.run(|frame| {
        input.step_with_window_events(
            &frame
                .events
                .iter()
                .filter_map(|event| {
                    if let Event::WindowEvent { event, .. } = event {
                        Some(event.clone())
                    } else {
                        None
                    }
                })
                .collect::<Box<_>>(),
        );

        // Hold the tab key to render in non-multisample mode
        let will_render_msaa = !input.key_held(KeyCode::Tab) && sample_count != SampleCount::Type1;

        angle += input
            .delta_time()
            .map(|dt| dt.as_secs_f32())
            .unwrap_or(0.016)
            * 0.1;
        let world_transform = Mat4::from_rotation_x(angle)
            * Mat4::from_rotation_y(angle * 0.61)
            * Mat4::from_rotation_z(angle * 0.22);

        let mut scene_uniform_buf = pool
            .lease(BufferInfo::host_mem(
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
                _pad: 0,
            }),
        );

        let cube_vertex_buf = frame.render_graph.bind_node(&cube_mesh.vertex_buf);
        let scene_uniform_buf = frame.render_graph.bind_node(scene_uniform_buf);

        let mut pass = frame
            .render_graph
            .begin_pass("cube")
            .bind_pipeline(if will_render_msaa {
                &mesh_msaa_pipeline
            } else {
                &mesh_noaa_pipeline
            })
            .set_depth_stencil(DepthStencilMode::DEPTH_WRITE)
            .access_node(cube_vertex_buf, AccessType::VertexBuffer)
            .access_descriptor(0, scene_uniform_buf, AccessType::AnyShaderReadUniformBuffer);

        if will_render_msaa {
            let msaa_color_image = pass.bind_node(
                pool.lease(
                    ImageInfo::image_2d(
                        frame.width,
                        frame.height,
                        pass.node_info(frame.swapchain_image).fmt,
                        vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                    )
                    .to_builder()
                    .sample_count(sample_count),
                )
                .unwrap(),
            );
            let msaa_depth_image = pass.bind_node(
                pool.lease(
                    ImageInfo::image_2d(
                        frame.width,
                        frame.height,
                        depth_format,
                        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                    )
                    .to_builder()
                    .sample_count(sample_count),
                )
                .unwrap(),
            );

            // Attachments for multisample mode
            pass = pass
                .clear_color_value(0, msaa_color_image, WHITE)
                .clear_depth_stencil(msaa_depth_image)
                .resolve_color(0, 1, frame.swapchain_image);
        } else {
            let noaa_depth_image = pass.bind_node(
                pool.lease(ImageInfo::image_2d(
                    frame.width,
                    frame.height,
                    depth_format,
                    vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
                ))
                .unwrap(),
            );

            // Attachments for non-multisample mode
            pass = pass
                .clear_color_value(0, frame.swapchain_image, WHITE)
                .clear_depth_stencil(noaa_depth_image)
                .store_color(0, frame.swapchain_image);
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

fn best_depth_format(device: &Device) -> vk::Format {
    for format in [vk::Format::D32_SFLOAT, vk::Format::D16_UNORM] {
        let format_props = Device::image_format_properties(
            device,
            format,
            vk::ImageType::TYPE_2D,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            vk::ImageCreateFlags::empty(),
        );

        if format_props.is_ok() {
            return format;
        }
    }

    panic!("Unsupported depth format");
}

fn max_supported_sample_count(device: &Device) -> SampleCount {
    let Vulkan10Limits {
        framebuffer_color_sample_counts,
        framebuffer_depth_sample_counts,
        ..
    } = device.physical_device.properties_v1_0.limits;
    match framebuffer_color_sample_counts & framebuffer_depth_sample_counts {
        s if s.contains(vk::SampleCountFlags::TYPE_64) => SampleCount::Type64,
        s if s.contains(vk::SampleCountFlags::TYPE_32) => SampleCount::Type32,
        s if s.contains(vk::SampleCountFlags::TYPE_16) => SampleCount::Type16,
        s if s.contains(vk::SampleCountFlags::TYPE_8) => SampleCount::Type8,
        s if s.contains(vk::SampleCountFlags::TYPE_4) => SampleCount::Type4,
        s if s.contains(vk::SampleCountFlags::TYPE_2) => SampleCount::Type2,
        s if s.contains(vk::SampleCountFlags::TYPE_1) => {
            warn!("MSAA not supported");

            SampleCount::Type1
        }
        _ => panic!("unsupported color/depth msaa"),
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
    sample_count: SampleCount,
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
            uint pad;
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

    let info = GraphicPipelineInfoBuilder::default().samples(sample_count);

    Ok(Arc::new(GraphicPipeline::create(
        device,
        info,
        [
            Shader::new_vertex(vert.as_slice()),
            Shader::new_fragment(frag.as_slice()),
        ],
    )?))
}

#[derive(Parser)]
struct Args {
    /// Enable Vulkan SDK validation layers
    #[arg(long)]
    debug: bool,
}

struct Model {
    vertex_buf: Arc<Buffer>,
    vertex_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, NoUninit)]
struct SceneUniformBuffer {
    view: Mat4,
    projection: Mat4,
    light_dir: Vec3,
    _pad: u32,
}
