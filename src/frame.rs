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

pub struct FrameContext<'a, P>
where
    P: SharedPointerKind,
{
    pub device: &'a Shared<Device<P>, P>,
    pub dt: f32,
    pub render_graph: &'a mut RenderGraph<P>,
    pub resolution: UVec2,
    pub events: &'a [Event<'a, ()>],
    pub swapchain: SwapchainImageNode<P>,
    pub will_exit: &'a mut bool,
    pub window: &'a Window,
}

impl<P> FrameContext<'_, P>
where
    P: SharedPointerKind,
{
    pub fn render_aspect_ratio(&self) -> f32 {
        let render_extent = self.resolution.as_vec2();
        render_extent.x / render_extent.y
    }

    pub fn center_cursor(&self) {
        let window_size = self.window.inner_size();
        let position = uvec2(window_size.width, window_size.height) / 2;
        self.move_cursor(&position);
    }

    pub fn move_cursor(&self, position: &UVec2) {
        let position = position.as_ivec2();
        let position = PhysicalPosition::new(position.x, position.y);
        self.window
            .set_cursor_position(position)
            .unwrap_or_default();
    }
}
