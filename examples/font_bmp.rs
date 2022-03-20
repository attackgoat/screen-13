use {screen_13::prelude_arc::*, screen_13_fx::prelude_arc::*, std::env::current_exe};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // Standard Screen-13 stuff
    let event_loop = EventLoop::new().build()?;
    let display = ComputePresenter::new(&event_loop.device)?;
    let mut image_loader = ImageLoader::new(&event_loop.device)?;
    let mut pak = open_fonts_pak()?;
    let mut pool = HashPool::new(&event_loop.device);

    // Load a bitmapped font from the pre-packed data file (must run the "bake_pak" example first)
    let small_10px_font = BitmapFont::load(
        pak.read_bitmap_font_key("font/small/small_10px")?,
        &mut image_loader,
    )?;

    // Create a renderer we can use to draw this font onto some image
    let text = BitmapFontRenderer::new(&event_loop.device)?;

    event_loop.run(|frame| {
        let image_node = frame.render_graph.bind_node(
            pool.lease(
                ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, frame.resolution)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
            )
            .unwrap(),
        );
        clear_color_node(frame.render_graph, image_node, 0.0, 0.0, 0.0, 1.0);
        text.render(
            frame.render_graph,
            image_node,
            &small_10px_font,
            IVec2::ZERO,
            "Hello, world!",
        );
        display.present_image(frame.render_graph, image_node, frame.swapchain);
    })?;

    Ok(())
}

fn open_fonts_pak() -> anyhow::Result<PakBuf> {
    let mut pak = current_exe()?;
    pak.set_file_name("fonts.pak");

    let pak = PakBuf::open(pak)?;

    Ok(pak)
}
