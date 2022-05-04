use screen_13::prelude_arc::*;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let demo = EventLoop::new().ray_tracing(true).build()?;

    demo.run(|frame| {
        frame.render_graph.clear_color_image(frame.swapchain_image);
    })?;

    Ok(())
}