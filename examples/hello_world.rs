use screen_13::prelude_arc::{run, DisplayError};

/// This example requires a color graphics adapter.
fn main() -> Result<(), DisplayError> {
    // The `run` function is shorthand for creating a default window and displaying it. See the
    // other examples for actual window usage - this would be for one-liner demos only.
    run(|frame| {
        frame
            .render_graph
            .clear_color_image_value(frame.swapchain_image, [100u8, 149, 237, 255]);
    })
}
