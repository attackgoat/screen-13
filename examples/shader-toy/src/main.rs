pub mod pak {
    use super::PakBuf;

    // The ".pak" file is a data transport type with compression and other useful features
    // It is used to hold the images used by this example, because they *could* be really
    // big - anyways we generated some bindings to make accessing those less error-prone:
    include!(concat!(env!("OUT_DIR"), "/pak_bindings.rs"));

    // This happens if you want the .pak bytes inside the executable itself
    #[cfg(feature = "include-pak")]
    pub fn open() -> anyhow::Result<PakBuf> {
        Ok(include_bytes!(concat!(env!("OUT_DIR"), "/data.pak"))
            .as_slice()
            .into())
    }

    // This happens if you want the .pak as a file next to the executable
    #[cfg(not(feature = "include-pak"))]
    pub fn open() -> anyhow::Result<PakBuf> {
        use std::env::current_exe;

        let mut pak_path = current_exe()?;
        pak_path.set_file_name("data.pak");

        Ok(PakBuf::open(pak_path)?)
    }
}

mod res {
    pub mod shader {
        include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
    }
}

use {anyhow::Context, screen_13::prelude_arc::*, screen_13_fx::*, std::time::Instant};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new()
        .debug(false)
        .desired_swapchain_image_count(3)
        .window(|builder| builder.with_inner_size(LogicalSize::new(1280.0f64, 720.0f64)))
        .build()
        .context("Event loop")?;
    let display = GraphicPresenter::new(&event_loop.device).context("Presenter")?;
    let mut cache = HashPool::new(&event_loop.device);
    let mut image_loader = ImageLoader::new(&event_loop.device).context("Loader")?;

    // Load source images: PakBuf -> BitmapBuf -> ImageBinding (here) -> ImageNode (during loop)
    let mut data = pak::open().context("Pak")?;
    let mut flowers_image_binding = Some(
        image_loader
            .decode_linear(
                &data
                    .read_bitmap(pak::IMAGE_FLOWERS_JPG)
                    .context("Unable to read flowers bitmap")?,
            )
            .context("Unable to decode flowers bitmap")?,
    );
    let mut noise_image_binding = Some(
        image_loader
            .decode_linear(
                &data
                    .read_bitmap(pak::IMAGE_RGBA_NOISE_PNG)
                    .context("Unable to read noise bitmap")?,
            )
            .context("Unable to decode noise bitmap")?,
    );

    // The shader toy example used two graphics pipelines with defaults:
    // no depth/stencil
    // 1x sample count
    // one-sided
    let buffer_pipeline = Shared::new(
        GraphicPipeline::create(
            &event_loop.device,
            GraphicPipelineInfo::default(),
            [
                Shader::new_vertex(res::shader::QUAD_VERT),
                Shader::new_fragment(res::shader::FLOCKAROO_BUF_FRAG),
            ],
        )
        .context("FLOCKAROO_BUF_FRAG")?,
    );
    let image_pipeline = Shared::new(
        GraphicPipeline::create(
            &event_loop.device,
            GraphicPipelineInfo::default(),
            [
                Shader::new_vertex(res::shader::QUAD_VERT),
                Shader::new_fragment(res::shader::FLOCKAROO_IMG_FRAG),
            ],
        )
        .context("FLOCKAROO_IMG_FRAG")?,
    );

    let mut render_graph = RenderGraph::new();
    let blank_image = render_graph.bind_node(
        cache
            .lease(
                ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, 8, 8)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST),
            )
            .context("Blank image")?,
    );

    let (width, height) = (event_loop.width(), event_loop.height());
    let framebuffer_image = render_graph.bind_node(
        cache
            .lease(
                ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, width, height).usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                ),
            )
            .context("Framebuffer image")?,
    );
    let temp_image = render_graph.bind_node(
        cache
            .lease(
                ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, width, height).usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                ),
            )
            .context("Temp image")?,
    );

    render_graph
        .clear_color_image_value(framebuffer_image, [1.0, 1.0, 0.0, 1.0])
        .clear_color_image_value(blank_image, [0.0, 0.0, 0.0, 1.0])
        .clear_color_image_value(temp_image, [0.0, 1.0, 0.0, 1.0]);

    let mut framebuffer_image_binding = Some(render_graph.unbind_node(framebuffer_image));
    let mut blank_image_binding = Some(render_graph.unbind_node(blank_image));
    let mut temp_image_binding = Some(render_graph.unbind_node(temp_image));

    render_graph.resolve().submit(&mut cache)?;

    let started_at = Instant::now();
    let mut mouse_buf = MouseBuf::default();
    let mut count = 0i32;
    let framebuffer_info = *framebuffer_image_binding.as_ref().unwrap().info();
    let flowers_image_info = *flowers_image_binding.as_ref().unwrap().info();
    let noise_image_info = *noise_image_binding.as_ref().unwrap().info();
    let blank_image_info = *blank_image_binding.as_ref().unwrap().info();

    event_loop
        .run(|frame| {
            // Update the stuff any shader toy shader would want to know each frame
            let elapsed = Instant::now() - started_at;
            update_mouse(&mut mouse_buf, frame.events);
            count += 1;

            // Bind things to this graph (the graph will own our things until we unbind them)
            let flowers_image = frame
                .render_graph
                .bind_node(flowers_image_binding.take().unwrap());
            let noise_image = frame
                .render_graph
                .bind_node(noise_image_binding.take().unwrap());
            let framebuffer_image = frame
                .render_graph
                .bind_node(framebuffer_image_binding.take().unwrap());
            let blank_image = frame
                .render_graph
                .bind_node(blank_image_binding.take().unwrap());
            let temp_image = frame
                .render_graph
                .bind_node(temp_image_binding.take().unwrap());

            // We need to push a shader-toy defined set of constants to each pipeline - any copy
            // type will do but we are getting fancy here by defining a struct to be super precise
            // about what we're doing - but you may want to just send a bunch of f32's
            #[repr(C)]
            #[derive(Clone, Copy)]
            struct PushConstants {
                resolution: Vec3,
                _pad_1: u32,
                date: Vec4,
                mouse: Vec4,
                time: f32,
                time_delta: f32,
                frame: i32,
                sample_rate: f32,
                channel_time: [f32; 4],
                channel_resolution: [Vec4; 4],
            }

            // Each pipeline gets the same constant data
            let push_consts = PushConstants {
                resolution: vec3(frame.width as _, frame.height as _, 1.0),
                _pad_1: Default::default(),
                date: vec4(1970.0, 1.0, 1.0, elapsed.as_secs_f32()),
                mouse: vec4(
                    if mouse_buf.any_held() {
                        mouse_buf.position().x
                    } else {
                        0.0
                    },
                    if mouse_buf.any_held() {
                        mouse_buf.position().y
                    } else {
                        0.0
                    },
                    mouse_buf.is_held(MouseButton::Left) as usize as f32,
                    mouse_buf.is_held(MouseButton::Right) as usize as f32,
                ),
                time: elapsed.as_secs_f32(),
                time_delta: frame.dt,
                frame: count,
                sample_rate: 44100.0,
                channel_time: [
                    elapsed.as_secs_f32(),
                    elapsed.as_secs_f32(),
                    elapsed.as_secs_f32(),
                    elapsed.as_secs_f32(),
                ],
                channel_resolution: [
                    framebuffer_info.extent.as_vec3().extend(1.0),
                    noise_image_info.extent.as_vec3().extend(1.0),
                    flowers_image_info.extent.as_vec3().extend(1.0),
                    blank_image_info.extent.as_vec3().extend(1.0),
                ],
            };

            let (input, output) = if count % 2 == 0 {
                (framebuffer_image, temp_image)
            } else {
                (temp_image, framebuffer_image)
            };

            // Fill a buffer using a single-pass CFD pipeline where previous output feeds next input
            frame
                .render_graph
                .begin_pass("Buffer A")
                .bind_pipeline(&buffer_pipeline)
                .read_descriptor(0, input)
                .read_descriptor(1, noise_image)
                .read_descriptor(2, flowers_image)
                .read_descriptor(3, blank_image)
                .store_color(0, output)
                .record_subpass(move |subpass| {
                    subpass.push_constants(push_consts);
                    subpass.draw(6, 1, 0, 0);
                });

            // Make the CFD look more like paint with a second pass
            frame
                .render_graph
                .begin_pass("Image")
                .bind_pipeline(&image_pipeline)
                .read_descriptor(0, output)
                .store_color(0, input)
                .record_subpass(move |subpass| {
                    subpass.push_constants(push_consts);
                    subpass.draw(6, 1, 0, 0);
                });

            // Done!
            display.present_image(frame.render_graph, input, frame.swapchain);

            // Unbind things from this graph (we want them back for the next frame!)
            flowers_image_binding = Some(frame.render_graph.unbind_node(flowers_image));
            noise_image_binding = Some(frame.render_graph.unbind_node(noise_image));
            framebuffer_image_binding = Some(frame.render_graph.unbind_node(framebuffer_image));
            blank_image_binding = Some(frame.render_graph.unbind_node(blank_image));
            temp_image_binding = Some(frame.render_graph.unbind_node(temp_image));
        })
        .context("Unable to run event loop")?;

    Ok(())
}
