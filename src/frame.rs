use {
    crate::{
        driver::Device,
        graph::{RenderGraph, SwapchainImageNode},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    glam::{uvec2, UVec2},
    winit::{dpi::PhysicalPosition, event::Event, window::Window},
};

pub fn center_cursor(window: &Window) {
    let window_size = window.inner_size();
    let position = uvec2(window_size.width, window_size.height) / 2;
    move_cursor(window, &position);
}

pub fn move_cursor(window: &Window, position: &UVec2) {
    let position = position.as_ivec2();
    let position = PhysicalPosition::new(position.x, position.y);
    window.set_cursor_position(position).unwrap_or_default();
}

pub struct FrameContext<'a, P>
where
    P: SharedPointerKind,
{
    pub device: &'a Shared<Device<P>, P>,
    pub dt: f32,
    pub events: &'a [Event<'a, ()>],
    pub height: u32,
    pub render_graph: &'a mut RenderGraph<P>,
    pub swapchain: SwapchainImageNode<P>,
    pub will_exit: &'a mut bool,
    pub width: u32,
    pub window: &'a Window,
}

impl<P> FrameContext<'_, P>
where
    P: SharedPointerKind,
{
    pub fn render_aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    pub fn center_cursor(&self) {
        center_cursor(self.window);
    }

    pub fn move_cursor(&self, position: &UVec2) {
        move_cursor(self.window, position);
    }
}
