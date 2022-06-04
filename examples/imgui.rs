use {
    screen_13::prelude::*,
    screen_13_fx::*,
    screen_13_imgui::{imgui, Condition, ImGui},
};

fn main() -> Result<(), DisplayError> {
    // Set RUST_LOG=trace in your environment variables to see log output
    pretty_env_logger::init();

    // Screen 13 things we need for this demo
    let event_loop = EventLoop::new().build()?;
    let display = ComputePresenter::new(&event_loop.device)?;
    let mut imgui = ImGui::new(&event_loop.device);
    let mut pool = HashPool::new(&event_loop.device);

    // Some example state to make the demo more interesting
    let mut value = 0;
    let choices = ["test test this is 1", "test test this is 2"];

    event_loop.run(|mut frame| {
        // Lease and clear an image as a stand-in for some real game or program output
        let app_image = frame.render_graph.bind_node(
            pool.lease(ImageInfo::new_2d(
                vk::Format::R8G8B8A8_UNORM,
                frame.width,
                frame.height,
                vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::STORAGE
                    | vk::ImageUsageFlags::TRANSFER_DST,
            ))
            .unwrap(),
        );
        frame
            .render_graph
            .clear_color_image_value(app_image, [0.2, 0.22, 0.2, 1.0]);

        // Use the draw function callback to do some fun meant-for-debug-mode GUI stuff
        let gui_image = imgui.draw_frame(&mut frame, |ui| {
            imgui::Window::new("Hello world")
                .position([10.0, 10.0], Condition::FirstUseEver)
                .size([340.0, 250.0], Condition::FirstUseEver)
                .build(ui, || {
                    ui.text_wrapped("Hello world!");
                    ui.text_wrapped("こんにちは世界！");
                    if ui.button(choices[value]) {
                        value += 1;
                        value %= 2;
                    }

                    ui.button("This...is...imgui-rs!");
                    ui.separator();
                    let mouse_pos = ui.io().mouse_pos;
                    ui.text(format!(
                        "Mouse Position: ({:.1},{:.1})",
                        mouse_pos[0], mouse_pos[1]
                    ));
                });
        });

        // Present "gui_image" on top of "app_image" onto "frame.swapchain"
        display.present_images(
            frame.render_graph,
            gui_image,
            app_image,
            frame.swapchain_image,
        );
    })
}
