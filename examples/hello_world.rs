use {screen_13::prelude_arc::*, screen_13_fx::*};

fn main() -> Result<(), DisplayError> {
    // Set RUST_LOG=trace in your environment variables to see log output
    pretty_env_logger::init();

    // The event loop creates a GPU device and operating system window which "display" will paint
    let event_loop = EventLoop::new().build()?;
    let display = GraphicPresenter::new(&event_loop.device)?;

    // Create a single owned image (we could instead lease one; see the shader-toy example)
    let mut image_binding = Some(
        event_loop.device.new_image(
            ImageInfo::new_2d(vk::Format::R8G8B8A8_SRGB, 10, 10)
                .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST),
        ),
    );

    event_loop.run(|frame| {
        // Take our image from the main() function and make it part of the render graph
        let image_node = frame.render_graph.bind_node(image_binding.take().unwrap());

        // The image is now a node which is just a usize and can be used in all parts of a graph
        frame.render_graph.clear_color_image_scalar(
            image_node,
            100.0 / 255.0,
            149.0 / 255.0,
            237.0 / 255.0,
            1.0,
        );

        // Run a vertex+pixel shader over image and stores into the swapchain image
        display.present_image(frame.render_graph, image_node, frame.swapchain);

        // Take the image from the graph and give it back to main() so we have it for the next loop
        image_binding = Some(frame.render_graph.unbind_node(image_node));
    })
}
