use {anyhow::Context, screen_13::prelude_arc::*, screen_13_fx::prelude_arc::*};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // Standard Screen 13 stuff
    let event_loop = EventLoop::new().build()?;
    let display = GraphicPresenter::new(&event_loop.device)?;
    let mut image_loader = ImageLoader::new(&event_loop.device)?;
    let mut pool = HashPool::new(&event_loop.device);

    // This example requires the "bake_pak" example to be run first
    let mut pak =
        PakBuf::open("fonts.pak").context("Pak file missing - run the bake_pak example first")?;

    // Load a bitmapped font from the pre-packed data file
    let small_10px_font = BitmapFont::load(
        pak.read_bitmap_font("font/small/small_10px")?,
        &mut image_loader,
    )?;

    event_loop.run(|frame| {
        let image_node = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d(
                vk::Format::R8G8B8A8_SRGB,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST,
            ))
            .unwrap(),
        );
        frame
            .render_graph
            .clear_color_image_value(image_node, [0, 0, 1, 1]);

        let text = "Hello, world!";
        let (_offset, extent) = small_10px_font.measure(text);
        let scale = 4.0;
        let position = vec2(
            frame.width as f32 * 0.5 / scale - extent.x as f32 * 0.5,
            frame.height as f32 * 0.5 / scale - extent.y as f32 * 0.5,
        );
        let color = [1.0, 1.0, 1.0];

        small_10px_font.print_scale(frame.render_graph, image_node, position, color, text, scale);

        display.present_image(frame.render_graph, image_node, frame.swapchain_image);
    })?;

    Ok(())
}
