use {
    anyhow::Context, screen_13::prelude_arc::*, screen_13_fx::prelude_arc::*, std::env::current_exe,
};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // Standard Screen-13 stuff
    let event_loop = EventLoop::new().debug(false).build()?;
    let display = GraphicPresenter::new(&event_loop.device)?;
    let mut image_loader = ImageLoader::new(&event_loop.device)?;
    let mut pool = HashPool::new(&event_loop.device);

    // This example requires the "bake_pak" example to be run first
    let mut pak = open_fonts_pak().context("Pak file missing - run the bake_pak example first")?;

    // Load a bitmapped font from the pre-packed data file
    let small_10px_font = BitmapFont::load(
        pak.read_bitmap_font_key("font/small/small_10px")?,
        &mut image_loader,
    )?;

    event_loop.run(|frame| {
        let image_node = frame.render_graph.bind_node(
            pool.lease(
                ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, frame.resolution).usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_DST,
                ),
            )
            .unwrap(),
        );
        clear_color_node(frame.render_graph, image_node, 0.0, 0.0, 1.0, 1.0);
        small_10px_font.print(
            frame.render_graph,
            image_node,
            Vec2::ZERO,
            [1.0, 1.0, 1.0],
            "Hello, world!",
        );
        display.present_image(frame.render_graph, image_node, frame.swapchain);
    })?;

    Ok(())
}

fn open_fonts_pak() -> anyhow::Result<PakBuf> {
    let mut pak_path = current_exe()?;
    pak_path.set_file_name("fonts.pak");

    let pak = PakBuf::open(pak_path)?;

    Ok(pak)
}
