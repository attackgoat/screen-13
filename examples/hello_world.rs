mod profile_with_puffin;

use screen_13::{DisplayError, EventLoop};

/// This example requires a color graphics adapter.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();
    profile_with_puffin::init();

    EventLoop::new().build()?.run(|frame| {
        frame
            .render_graph
            .clear_color_image_value(frame.swapchain_image, [100u8, 149, 237, 255]);
    })
}
