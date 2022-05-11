use screen_13::prelude_arc::*;

fn main() {
    run(|mut frame| {
        let image = Image::create(
            frame.device,
            ImageInfo::new_3d(
                vk::Format::R16_UINT,
                1103,
                872,
                493,
                vk::ImageUsageFlags::STORAGE,
            )
            .mip_level_count(6),
        )
        .unwrap();

        frame.render_graph.clear_color_image(frame.swapchain_image);

        frame.exit();
    })
    .unwrap();
}
