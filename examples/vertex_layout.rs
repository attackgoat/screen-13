use {
    bytemuck::{cast_slice, Pod, Zeroable},
    inline_spirv::inline_spirv,
    screen_13::prelude::*,
    std::{mem::size_of, sync::Arc},
};

fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    // NOTE: This example uses the 64-bit rules defined in the Vulkan spec, they're not obvious:
    // https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#fxvertex-attrib

    let event_loop = EventLoop::new().debug(true).build()?;

    let automatic_layout_pipeline = create_automatic_layout_pipeline(&event_loop.device)?;
    let manual_layout_pipeline = create_manual_layout_pipeline(&event_loop.device)?;

    let f32_vertex_buf = Arc::new(Buffer::create_from_slice(
        &event_loop.device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&[
            // Vertex 0
            -1f32, -1.0, 1.0, 0.0, 0.0, // vec2 position + vec3 color
            // Vertex 1
            -1.0, 1.0, 0.0, 1.0, 0.0, // vec2 position + vec3 color
            // Vertex 2
            1.0, 1.0, 0.0, 0.0, 1.0, // vec2 position + vec3 color
        ]),
    )?);

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct Vertex64([f64; 2], [f32; 3], u32);

    let f64_vertex_buf = Arc::new(Buffer::create_from_slice(
        &event_loop.device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        cast_slice(&[
            Vertex64([-1.0, -1.0], [1.0, 0.0, 0.0], 0), // vec2 position + vec3 color + pad
            Vertex64([1.0, 1.0], [0.0, 0.0, 1.0], 0),   // vec2 position + vec3 color + pad
            Vertex64([1.0, -1.0], [0.0, 1.0, 0.0], 0),  // vec2 position + vec3 color + pad
        ]),
    )?);

    event_loop.run(|frame| {
        let f32_vertex_buf = frame.render_graph.bind_node(&f32_vertex_buf);
        let f64_vertex_buf = frame.render_graph.bind_node(&f64_vertex_buf);

        frame
            .render_graph
            .begin_pass("Automatically-defined 32-bit vertex layout")
            .bind_pipeline(&automatic_layout_pipeline)
            .clear_color(0, frame.swapchain_image)
            .store_color(0, frame.swapchain_image)
            .access_node(f32_vertex_buf, AccessType::VertexBuffer)
            .record_subpass(move |subpass, _| {
                subpass.bind_vertex_buffer(f32_vertex_buf).draw(3, 1, 0, 0);
            });

        // (Fun fact: Screen 13 turns these two passes into one renderpass with a second subpass!)

        frame
            .render_graph
            .begin_pass("Manually-defined 64-bit vertex layout")
            .bind_pipeline(&manual_layout_pipeline)
            .store_color(0, frame.swapchain_image)
            .access_node(f64_vertex_buf, AccessType::VertexBuffer)
            .record_subpass(move |subpass, _| {
                subpass.bind_vertex_buffer(f64_vertex_buf).draw(3, 1, 0, 0);
            });
    })
}

fn assert_64bit_supported(device: &Arc<Device>) {
    unsafe {
        assert!(device
            .instance
            .get_physical_device_format_properties(
                *device.physical_device,
                vk::Format::R64G64_SFLOAT
            )
            .buffer_features
            .contains(vk::FormatFeatureFlags::VERTEX_BUFFER));
    }
}

fn create_vertex_shader(is_f64: bool) -> ShaderBuilder {
    // From the specs: Input attributes which have three- or four-component 64-bit formats will
    // consume two consecutive locations
    //
    // To support a vec3 64-bit case this means color_in needs to be on location 2

    // This shader is compiled with a macro because we want to be able to switch the vec2 type to a
    // dvec2 when using 64-bit positions; and for the purposes of this example we don't want to
    // duplicate this shader code. You probably don't want to do this, or you may have different
    // facilities for generating SPIR-V code - either way ignore the macro unless you're interested
    // in the inline_spirv! wizardry it contains which is unrelated to this example.
    macro_rules! compile_vert {
        ($vec2_ty:literal) => {
            inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) in VEC2_TY position_in;
            layout(location = 1) in vec3 color_in;

            layout(location = 0) out vec3 color_out;

            void main() {
                gl_Position = vec4(position_in, 0, 1);
                color_out = color_in;
            }
            "#,
            vert,
            D VEC2_TY = $vec2_ty,
        )};
    }

    let spirv = if is_f64 {
        compile_vert!("dvec2").as_slice()
    } else {
        compile_vert!("vec2").as_slice()
    };

    Shader::new_vertex(spirv)
}

fn create_automatic_layout_pipeline(
    device: &Arc<Device>,
) -> Result<Arc<GraphicPipeline>, DriverError> {
    let vertex = create_vertex_shader(false);

    create_pipeline(device, vertex)
}

fn create_manual_layout_pipeline(
    device: &Arc<Device>,
) -> Result<Arc<GraphicPipeline>, DriverError> {
    assert_64bit_supported(device);

    let position_size = 2 * size_of::<f64>() as u32;
    let color_size = 3 * size_of::<f32>() as u32;
    let pad_size = size_of::<u32>() as u32;

    let vertex = create_vertex_shader(true).vertex_layout(
        &[vk::VertexInputBindingDescription {
            binding: 0,
            stride: position_size + color_size + pad_size,
            input_rate: vk::VertexInputRate::VERTEX,
        }],
        &[
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R64G64_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: position_size,
            },
        ],
    );

    create_pipeline(device, vertex)
}

fn create_pipeline(
    device: &Arc<Device>,
    vertex: ShaderBuilder,
) -> Result<Arc<GraphicPipeline>, DriverError> {
    let fragment_spirv = inline_spirv!(
        r#"
        #version 460 core

        layout(location = 0) in vec3 color_in;

        layout(location = 0) out vec4 color_out;

        void main() {
            color_out = vec4(color_in, 1.0);
        }
        "#,
        frag
    );

    Ok(Arc::new(GraphicPipeline::create(
        device,
        GraphicPipelineInfo::default(),
        [vertex, Shader::new_fragment(fragment_spirv.as_slice())],
    )?))
}
