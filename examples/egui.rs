use {screen_13::prelude::*, screen_13_egui::prelude::*};

fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new()
        .desired_swapchain_image_count(2)
        .desired_surface_format(Surface::linear_or_default)
        .window(|window| window.with_transparent(false))
        .build()?;
    let mut egui = Egui::new(&event_loop.device, event_loop.as_ref());

    let mut cache = LazyPool::new(&event_loop.device);

    event_loop.run(|frame| {
        let img = frame.render_graph.bind_node(
            cache
                .lease(ImageInfo::image_2d(
                    100,
                    100,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                ))
                .unwrap(),
        );
        frame
            .render_graph
            .clear_color_image_value(img, [0., 1., 0., 1.]);
        frame
            .render_graph
            .clear_color_image_value(frame.swapchain_image, [0., 0., 0., 1.]);

        let id = egui.register_texture(img);

        egui.run(
            frame.window,
            frame.events,
            frame.swapchain_image,
            frame.render_graph,
            |ui| {
                egui::Window::new("Test")
                    .resizable(true)
                    .vscroll(true)
                    .default_size([400., 400.])
                    .show(ui, |ui| {
                        ui.add(egui::Button::new("Test"));
                        ui.add(egui::Link::new("Test"));
                        ui.add(egui::Image::new((id, egui::Vec2::new(50., 50.))));
                    });
            },
        );
    })
}
