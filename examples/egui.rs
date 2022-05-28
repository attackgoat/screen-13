
use{
    screen_13::prelude_arc::*,
    screen_13_fx::*,
    screen_13_egui::*,
};

fn main() -> Result<(), DisplayError>{
    pretty_env_logger::init();

    let event_loop = EventLoop::new().build()?;
    let mut pool = HashPool::new(&event_loop.device);

    event_loop.run(|mut frame|{
        frame.render_graph.clear_color_image_value(frame.swapchain_image, [1., 0., 0., 1.]);
    })
}
