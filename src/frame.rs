use {
    crate::{
        driver::Device,
        graph::{node::SwapchainImageNode, RenderGraph},
    },
    std::sync::Arc,
    winit::{dpi::PhysicalPosition, event::Event, window::Window},
};

pub fn center_cursor(window: &Window) {
    let window_size = window.inner_size();
    let x = window_size.width / 2;
    let y = window_size.height / 2;
    set_cursor_position(window, x, y);
}

pub fn set_cursor_position(window: &Window, x: u32, y: u32) {
    let position = PhysicalPosition::new(x as i32, y as i32);
    window.set_cursor_position(position).unwrap_or_default();
}

pub struct FrameContext<'a> {
    pub device: &'a Arc<Device>,
    pub dt: f32,
    pub events: &'a [Event<'a, ()>],
    pub height: u32,
    pub render_graph: &'a mut RenderGraph,
    pub swapchain_image: SwapchainImageNode,
    pub will_exit: &'a mut bool,
    pub width: u32,
    pub window: &'a Window,
}

impl FrameContext<'_> {
    pub fn exit(&mut self) {
        *self.will_exit = true;
    }

    pub fn render_aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    pub fn center_cursor(&self) {
        center_cursor(self.window);
    }

    pub fn set_cursor_position(&self, x: u32, y: u32) {
        set_cursor_position(self.window, x, y);
    }
}
