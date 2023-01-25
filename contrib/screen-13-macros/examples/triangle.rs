use glam::Vec3;

use {
    bytemuck::{cast_slice, Pod, Zeroable},
    inline_spirv::inline_spirv,
    screen_13::prelude::*,
    screen_13_macros::prelude::*,
    std::sync::Arc,
};

// A Vulkan triangle using a graphic pipeline, vertex/fragment shaders, and index/vertex buffers.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    #[repr(C)]
    #[derive(Vertex, Pod, Zeroable, Copy, Clone, Default)]
    struct Vertex {
        #[format(R32G32B32_SFLOAT)]
        position: Vec3,
        #[format(R8G8B8_UNORM)]
        color: [u8; 3],
        _padding: u8,
    }

    let event_loop = EventLoop::new().build()?;
    let triangle_pipeline = Arc::new(GraphicPipeline::create(
        &event_loop.device,
        GraphicPipelineInfo::default(),
        [
            Shader::new_vertex(
                inline_spirv!(
                    r#"
                    #version 460 core

                    layout(location = 0) in vec3 position;
                    layout(location = 1) in vec3 color;

                    layout(location = 0) out vec3 vk_Color;

                    void main() {
                        gl_Position = vec4(position, 1);
                        vk_Color = color;
                    }
                    "#,
                    vert
                )
                .as_slice(),
            )
            .with_vertex_layout(Vertex::layout(vk::VertexInputRate::VERTEX)),
            Shader::new_fragment(
                inline_spirv!(
                    r#"
                    #version 460 core

                    layout(location = 0) in vec3 color;

                    layout(location = 0) out vec4 vk_Color;

                    void main() {
                        vk_Color = vec4(color, 1);
                    }
                    "#,
                    frag
                )
                .as_slice(),
            )
            .build(),
        ],
    )?);

    let index_buf = Arc::new(Buffer::create_from_slice(
        &event_loop.device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        cast_slice(&[0u16, 1, 2]),
    )?);

    let vertex_buf = Arc::new(Buffer::create_from_slice(
        &event_loop.device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&[
            Vertex {
                position: Vec3::new(1.0f32, 1.0, 0.0),
                color: [255, 0, 0],
                ..Default::default()
            },
            Vertex {
                position: Vec3::new(0.0, -1.0, 0.0),
                color: [0, 255, 0],
                ..Default::default()
            },
            Vertex {
                position: Vec3::new(-1.0, 1.0, 0.0),
                color: [0, 0, 255],
                ..Default::default()
            },
        ]),
    )?);

    event_loop.run(|frame| {
        let index_node = frame.render_graph.bind_node(&index_buf);
        let vertex_node = frame.render_graph.bind_node(&vertex_buf);

        frame
            .render_graph
            .begin_pass("Triangle Example")
            .bind_pipeline(&triangle_pipeline)
            .access_node(index_node, AccessType::IndexBuffer)
            .access_node(vertex_node, AccessType::VertexBuffer)
            .clear_color(0, frame.swapchain_image)
            .store_color(0, frame.swapchain_image)
            .record_subpass(move |subpass, _| {
                subpass.bind_index_buffer(index_node, vk::IndexType::UINT16);
                subpass.bind_vertex_buffer(vertex_node);
                subpass.draw_indexed(3, 1, 0, 0, 0);
            });
    })
}
