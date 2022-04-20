use {inline_spirv::inline_spirv, screen_13::prelude_arc::*};

fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let screen_13 = EventLoop::new().debug(true).build()?;
    let mut cache = HashPool::new(&screen_13.device);

    let triangle_pipeline = screen_13.device.new_graphic_pipeline(
        GraphicPipelineInfo::default(),
        [
            Shader::new_vertex(into_u8_slice(inline_spirv!(
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
            ))),
            Shader::new_fragment(into_u8_slice(inline_spirv!(
                r#"
                #version 460 core

                layout(location = 0) in vec3 color;

                layout(location = 0) out vec4 vk_Color;
                
                void main() {
                    vk_Color = vec4(color, 1);
                }
                "#,
                frag
            ))),
        ],
    );

    let mut index_buf = Some(BufferLeaseBinding({
        let mut buf = cache.lease(BufferInfo::new_mappable(
            12,
            vk::BufferUsageFlags::INDEX_BUFFER,
        ))?;
        Buffer::copy_from_slice(buf.get_mut().unwrap(), 0, into_u8_slice([0u32, 1, 2]));
        buf
    }));

    let mut vertex_buf = Some(BufferLeaseBinding({
        let mut buf = cache.lease(BufferInfo::new_mappable(
            72,
            vk::BufferUsageFlags::VERTEX_BUFFER,
        ))?;
        Buffer::copy_from_slice(
            buf.get_mut().unwrap(),
            0,
            into_u8_slice([
                1.0f32, 1.0, 0.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0,
                0.0, 0.0, 1.0,
            ]),
        );
        buf
    }));

    screen_13.run(|frame| {
        let index_node = frame.render_graph.bind_node(index_buf.take().unwrap());
        let vertex_node = frame.render_graph.bind_node(vertex_buf.take().unwrap());

        frame
            .render_graph
            .begin_pass("Triangle Example")
            .bind_pipeline(&triangle_pipeline)
            .read_node(index_node)
            .read_node(vertex_node)
            .clear_color_value(0, [1, 0, 1, 0])
            .store_color(0, frame.swapchain_image)
            .record_subpass(move |subpass| {
                subpass.bind_index_buffer(index_node, vk::IndexType::UINT32);
                subpass.bind_vertex_buffer(vertex_node);
                subpass.draw_indexed(3, 1, 0, 0, 0);
            });

        index_buf = Some(frame.render_graph.unbind_node(index_node));
        vertex_buf = Some(frame.render_graph.unbind_node(vertex_node));
    })
}
