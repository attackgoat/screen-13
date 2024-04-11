mod profile_with_puffin;

use {bytemuck::cast_slice, inline_spirv::inline_spirv, screen_13::prelude::*, std::sync::Arc};

// A Vulkan triangle using a graphic pipeline, vertex/fragment shaders, and index/vertex buffers.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();
    profile_with_puffin::init();

    let event_loop = EventLoop::new().build()?;
    let triangle_pipeline = Arc::new(GraphicPipeline::create(
        &event_loop.device,
        GraphicPipelineInfo::default(),
        [
            Shader::new_vertex(
                inline_spirv!(
                    r#"
                    #version 460 core
                    #extension GL_EXT_nonuniform_qualifier : require

                    layout(set = 0, binding = 0) uniform UBO {
                        float multiplier;
                    } ubo[];

                    layout(location = 0) in vec3 position;
                    layout(location = 1) in vec3 color;

                    layout(location = 0) out vec3 vk_Color;

                    void main() {
                        vec3 p = position;
                        p.y -= float(gl_InstanceIndex * 2 - 2);

                        gl_Position = vec4(p / 3, 1);
                        vk_Color = color * ubo[nonuniformEXT(gl_InstanceIndex)].multiplier;
                    }
                    "#,
                    vert
                )
                .as_slice(),
            ),
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
            ),
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
            1.0f32, 1.0, 0.0, // v1
            1.0, 0.0, 0.0, // red
            0.0, -1.0, 0.0, // v2
            0.0, 1.0, 0.0, // green
            -1.0, 1.0, 0.0, // v3
            0.0, 0.0, 1.0, // blue
        ]),
    )?);

    let uniforms = (1..4)
        .map(|n| {
            Ok(Arc::new(Buffer::create_from_slice(
                &event_loop.device,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                (n as f32 / 3.0).to_ne_bytes(),
            )?))
        })
        .collect::<Result<Box<_>, DriverError>>()?;

    event_loop.run(|frame| {
        let index_node = frame.render_graph.bind_node(&index_buf);
        let vertex_node = frame.render_graph.bind_node(&vertex_buf);

        let mut pass = frame
            .render_graph
            .begin_pass("Triangle Example")
            .bind_pipeline(&triangle_pipeline)
            .access_node(index_node, AccessType::IndexBuffer)
            .access_node(vertex_node, AccessType::VertexBuffer);

        for (idx, uniform) in uniforms.iter().enumerate() {
            let uniform = pass.bind_node(uniform);
            pass = pass.access_descriptor(
                (0, [idx as u32]),
                uniform,
                AccessType::VertexShaderReadUniformBuffer,
            );
        }

        pass.clear_color(0, frame.swapchain_image)
            .store_color(0, frame.swapchain_image)
            .record_subpass(move |subpass, _| {
                subpass.bind_index_buffer(index_node, vk::IndexType::UINT16);
                subpass.bind_vertex_buffer(vertex_node);
                subpass.draw_indexed(3, 3, 0, 0, 0);
            });
    })
}
