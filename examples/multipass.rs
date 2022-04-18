use {inline_spirv::inline_spirv, screen_13::prelude_arc::*};

// NOTE: When this example runs, there will be a blank screen - that's OK!

/// This example does no real work, but rather just uses the api in order to call a few
/// shader pipelines in a fun and realistic manner.
///
/// The key principle is that you can lease resources (images and buffers) and compose
/// rendering operations with just a few lines of RenderGraph builder-pattern code.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    // Create a bunch of "pipelines" (shader code setup to run on the GPU) - we keep these
    // around and just switch between which one we're using at any one point during a frame
    let event_loop = EventLoop::new().build().unwrap();
    let draw_funky_shape_deferred = create_draw_funky_shape_deferred_pipeline(&event_loop.device);
    let fill_quad_linear_gradient = create_fill_quad_linear_gradient_pipeline(&event_loop.device);

    // We also need a cache (this one is backed by a hashmap of resource info, fast but basic)
    // There will be more cache types later and traits exposed
    let mut cache = HashPool::new(&event_loop.device);

    // Static index/vertex data courtesy of the polyhedron-ops library
    let (indices, vertices) = funky_shape_triangle_mesh_buffers();
    let index_count = indices.len() as u32;
    let indices = into_u8_slice(&indices);
    let vertices = into_u8_slice(&vertices);

    // Pre-define some basic information structs we'll repeatedly use to acquire leased resources
    // (Usually we would do this at the place of use, but for clarity its outside the loop here)
    // (Note the event_loop height and width may change, and are provided in the frame context,
    // but we're not using that in this demo so the image won't resize with the window!
    let image_info = image_info_2d(event_loop.width(), event_loop.height());
    let index_buf_info = index_buffer_info(indices.len() as u64);
    let vertex_buf_info = vertex_buffer_info(vertices.len() as u64);

    // Some colors for readability
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 1.0];

    // Event loop runs the frame callback on the current thread
    event_loop.run(|frame| {
        // We are now rendering a frame for the provided swapchain image node and render graph.
        let graph = frame.render_graph;
        let swapchain_image = frame.swapchain;

        // Part 1: Get and prepare some resources - you could have Binding instances that are
        // bound to, used on, and then later unbound from a graph and repeated like that each
        // frame, or, you could lease things and let the magic of Arc<T> just handle it. Here
        // We lease things, and so we have to fill them freshly each time.

        // Lease + fill + bind a buffer: the questionably-readable three line way
        let mut index_buf = cache.lease(index_buf_info).unwrap();
        Buffer::mapped_slice_mut(index_buf.get_mut().unwrap())[0..indices.len()]
            .copy_from_slice(indices);
        let index_buf = graph.bind_node(index_buf);

        // Lease + fill a buffer: maybe a more sane looking way of doing it
        let vertex_buf = graph.bind_node({
            let mut vertex_buf = cache.lease(vertex_buf_info).unwrap();
            let data = Buffer::mapped_slice_mut(vertex_buf.get_mut().unwrap());
            data[0..vertices.len()].copy_from_slice(vertices);
            vertex_buf
        });

        // Lease a couple images (they may be blank or have pictures of cats in them but they are valid/ready)
        let image1 = graph.bind_node(cache.lease(image_info).unwrap());
        let image2 = graph.bind_node(cache.lease(image_info).unwrap());
        let image3 = graph.bind_node(cache.lease(image_info).unwrap());

        // You can also do this:
        let image1 = graph.bind_node({
            let mut img = cache.lease(image_info).unwrap();
            img.get_mut().unwrap().name = Some("image1".to_owned());
            img
        });
        let image2 = graph.bind_node({
            let mut img = cache.lease(image_info).unwrap();
            img.get_mut().unwrap().name = Some("image2".to_owned());
            img
        });
        let image3 = graph.bind_node({
            let mut img = cache.lease(image_info).unwrap();
            img.get_mut().unwrap().name = Some("image3".to_owned());
            img
        });

        // Part 2: Do things to the graph! Build passes where each pass contains:
        // - Access to nodes: declare either read/write/or specific access
        // - Pipeline configuration: tell it what depth settings and push constants to send
        // - Read descriptor bindings and load/store color values, have fun, yay!!

        // You can record two or more draws in a single pass; they inherit the draw state
        // from above calls. In this cas we reset the "store" between draws but we do not
        // bother resetting the "clear" state as you can see image2 will be cleared with
        // green also.
        graph
            .begin_pass("gradients")
            .bind_pipeline(&fill_quad_linear_gradient)
            .clear_color_array(0, green)
            .store_color(0, image1)
            .record_subpass(move |subpass| {
                subpass.push_constants((red, blue));
                subpass.draw(6, 1, 0, 0);
            })
            .store_color(0, image2)
            .record_subpass(move |subpass| {
                // We updated the constants and which attachment is getting stored, but otherwise same pipeline config here
                subpass.push_constants((green, blue));
                subpass.draw(6, 1, 0, 0);
            });

        // The above is "one pass" which logically happens first but physically may happen later
        // once the hardware schedules it - but it can't do that until we hand the graph over
        // at the bottom of the closure -> Screen 13 takes the graph and presents it to the swapchain
        // so long as we do something (transfer/write/compute) to the swapchain the correct operations
        // will be sent to the display. You just need to record some passes to the graph.

        // Alternatively to the above, you might just record two passes, bind two pipelines, etc. As long as they're setup
        // the same they will be trivially merged together or moved apart - whatever ends up being best. In the above case
        // because we didn't start a second "begin_pass" call, we are not allowing the GPU to break up this unit of work.
        // Maybe in general it's a good idea to record lots of short passes so the resolver code has more to work with.

        // Let's do some more work... This draws the funky shape into image3 stored as a deferred gbuffer ()
        graph
            .begin_pass("This text shows up in debuggers like RenderDoc")
            .bind_pipeline(&draw_funky_shape_deferred)
            .access_node(index_buf, AccessType::IndexBuffer) // We must call access on the buffers
            .access_node(vertex_buf, AccessType::VertexBuffer) // because we use them in a subpass
            .clear_color(0)
            .read_descriptor((0, [0]), image1) // We are declaring the read on image1 here
            .read_descriptor((0, [1]), image2) // and the second array item will be image2
            .store_color(0, image3) // and we declare we're writing the results to image3
            .record_subpass(move |subpass| {
                subpass
                    .push_constants((Mat4::IDENTITY, Vec4::ONE))
                    .bind_index_buffer(index_buf, vk::IndexType::UINT32)
                    .bind_vertex_buffer(vertex_buf)
                    .draw(index_count, 1, 0, 0);
            });

        // This will suffice as a way to get image3 presented - although you might want to check out the
        // included presenter types for more advanced display techniques. This issues a copy command at this
        // logical point in the graph - nothing is copied "yet" - it copies when the graph resolves later
        graph.copy_image(image3, swapchain_image);

        // Uncomment the last line if you want to instead draw a magenta screen.
        // NOTE: This will not cancel the above render passes; they will still run.
        //graph.clear_color_image(swapchain_image, 1.0, 0.0, 1.0, 1.0);
    })
}

const fn index_buffer_info(size: vk::DeviceSize) -> BufferInfo {
    BufferInfo {
        size,
        usage: vk::BufferUsageFlags::INDEX_BUFFER,
        can_map: true,
    }
}

const fn vertex_buffer_info(size: vk::DeviceSize) -> BufferInfo {
    BufferInfo {
        size,
        usage: vk::BufferUsageFlags::VERTEX_BUFFER,
        can_map: true,
    }
}

fn image_info_2d(width: u32, height: u32) -> ImageInfo {
    // Currently this is bad API you MUST specify usage of the image, but it's not part of the ctor
    ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, width, height)
        .usage(
            vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::INPUT_ATTACHMENT,
        )
        .build()
        .unwrap()

    // Additional builder functions that might be of interest:
    // .tiling(vk::ImageTiling::OPTIMAL)) <- Thinking about removing - LEAVE AT OPTIMAL ALWAYS
    // .mip_level_count(1)
    // .array_elements(1)
    // .sample_count(SampleCount::X1)
}

fn create_fill_quad_linear_gradient_pipeline(device: &Shared<Device>) -> Shared<GraphicPipeline> {
    let vertex_shader = Shader::new_vertex(into_u8_slice(inline_spirv!(
        r#"
        #version 460 core

        const vec3 position[6] = vec3[6](
            vec3(-1.0, 1.0, 0.0),
            vec3(-1.0, 0.0, 0.0),
            vec3(1.0, 0.0, 0.0),
            vec3(-1.0, 1.0, 0.0),
            vec3(1.0, 0.0, 0.0),
            vec3(1.0, 1.0, 0.0)
        );
        const vec2 tex_coord[6] = vec2[6](
            vec2(0.0, 0.0),
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0)
        );

        layout(location = 0) out vec2 vk_TexCoord;

        void main() {
            gl_Position = vec4(position[gl_VertexIndex], 1);
            vk_TexCoord = tex_coord[gl_VertexIndex];
        }
        "#,
        vert
    )));

    let fragment_shader = Shader::new_fragment(into_u8_slice(inline_spirv!(
        r#"
        #version 460 core

        layout(constant_id = 0) const int NUM_SAMPLERS = 1;

        layout(push_constant) uniform PushConstants {
            layout(offset = 0) vec4 start_color;
            layout(offset = 16) vec4 end_color;
        } push_constants;
        
        layout(location = 0) in vec2 tex_coord;

        layout(location = 0) out vec4 vk_Color;
        
        void main() {
            vk_Color = push_constants.start_color * vec4(tex_coord.x)
                     + push_constants.end_color * vec4(tex_coord.y);
        }
        "#,
        frag
    )));

    Shared::new(
        GraphicPipeline::create(
            device,
            GraphicPipelineInfo::new().blend(BlendMode::Alpha),
            [vertex_shader, fragment_shader],
        )
        .unwrap(),
    )
}

fn create_draw_funky_shape_deferred_pipeline(device: &Shared<Device>) -> Shared<GraphicPipeline> {
    let vertex_shader = Shader::new_vertex(into_u8_slice(inline_spirv!(
        r#"
        #version 460 core
        
        layout(push_constant) uniform PushConstants {
            layout(offset = 0) mat4 transform;
        } push_constants;
        
        layout(location = 0) in vec3 position;
        layout(location = 1) in vec2 tex_coord;
        layout(location = 2) in vec3 normal;
        
        layout(location = 0) out vec3 vk_Normal;
        layout(location = 1) out vec2 vk_TexCoord;
        
        void main() {
            gl_Position = push_constants.transform * vec4(position, 1);
            vk_Normal = normal;
            vk_TexCoord = tex_coord;
        }
        "#,
        vert
    )));

    let fragment_shader = Shader::new_fragment(into_u8_slice(inline_spirv!(
        r#"
        #version 460 core

        layout(constant_id = 0) const int NUM_SAMPLERS = 1;

        layout(push_constant) uniform PushConstants {
            layout(offset = 64) vec4 coolness_factor;
        } push_constants;
        
        layout(set = 0, binding = 0) uniform sampler2D sampler_llc[NUM_SAMPLERS];

        layout(location = 0) in vec3 normal;
        layout(location = 1) in vec2 tex_coord;

        layout(location = 0) out vec4 vk_Color;
        
        void main() {
            vk_Color = push_constants.coolness_factor;

            for (int idx = 0; idx < NUM_SAMPLERS; idx++) {
                vk_Color *= texture(sampler_llc[idx], tex_coord);
            }
        }
        "#,
        frag
    )))
    .specialization_info(SpecializationInfo::new(
        [vk::SpecializationMapEntry {
            constant_id: 0,
            offset: 0,
            size: 4,
        }],
        2u32.to_ne_bytes(), // <--- Specifies 2 for NUM_SAMPLERS
    ));

    // NOTE: The fragment shader above uses the `constant_id` layout on the `sampler_llc` sampler
    // array length bound; and so shader reflection that gets performed won't know how many
    // texture descriptors are really needed unless you tell the engine. For this type of case, we have
    // specialization info, above, which declares the data. This pipeline will now be hardcoded to 2.
    // If we do not set the specialization info like this then the shader would have 1 as the default
    // which is specified here: .....onst int NUM_SAMPLERS = 1;  <--- This value

    Shared::new(
        GraphicPipeline::create(
            device,
            GraphicPipelineInfo::default(),
            [vertex_shader, fragment_shader],
        )
        .unwrap(),
    )
}

/// Returns index buffer and position/tex-coord/normal buffer (polyhedron_ops you are 🥇🏆🥂💯)
fn funky_shape_triangle_mesh_buffers() -> (Vec<u32>, Vec<(Vec3, Vec2, Vec3)>) {
    let (indices, positions, normals) = polyhedron_ops::Polyhedron::dodecahedron()
        .chamfer(None, true)
        .propeller(None, true)
        .ambo(None, true)
        .gyro(None, None, true)
        .finalize()
        .to_triangle_mesh_buffers();
    let vertices = positions
        .into_iter()
        .zip(normals.into_iter())
        .map(|(position, normal)| {
            (
                vec3(position.x, position.y, position.z),
                vec2(
                    normal.x.atan2(normal.z) / std::f32::consts::FRAC_2_PI + 0.5,
                    normal.y * 0.5 + 0.5,
                ),
                vec3(normal.x, normal.y, normal.z),
            )
        })
        .collect();

    (indices, vertices)
}
