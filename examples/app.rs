use {
    screen_13::{
        driver::{
            device::{Device, DeviceInfo},
            surface::Surface,
            swapchain::{Swapchain, SwapchainInfo},
        },
        graph::RenderGraph,
        pool::hash::HashPool,
        Display,
    },
    std::sync::Arc,
    winit::{
        application::ApplicationHandler,
        error::EventLoopError,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::{Window, WindowId},
    },
};

fn main() -> Result<(), EventLoopError> {
    EventLoop::new()?.run_app(&mut Application::default())
}

#[derive(Default)]
struct Application(Option<Context>);

impl ApplicationHandler for Application {
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.0.as_ref().unwrap().window.request_redraw();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title("Screen 13");
        let window = event_loop.create_window(window_attributes).unwrap();
        let device = Arc::new(Device::create_presentable(DeviceInfo::default(), &window).unwrap());
        let display_pool = Box::new(HashPool::new(&device));
        let display = Display::new(&device, display_pool, 3, 0).unwrap();
        let surface = Surface::create(&device, &window).unwrap();
        let surface_formats = Surface::formats(&surface).unwrap();
        let surface_format = Surface::linear_or_default(&surface_formats);
        let window_size = window.inner_size();
        let swapchain = Swapchain::new(
            &device,
            surface,
            SwapchainInfo::new(window_size.width, window_size.height, surface_format),
        )
        .unwrap();

        self.0 = Some(Context {
            device,
            display,
            swapchain,
            window,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let context = self.0.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let mut swapchain_info = context.swapchain.info();
                swapchain_info.width = size.width;
                swapchain_info.height = size.height;
                context.swapchain.set_info(swapchain_info);
            }
            WindowEvent::RedrawRequested => {
                context.draw();
            }
            _ => (),
        }
    }
}

struct Context {
    device: Arc<Device>,
    display: Display,
    swapchain: Swapchain,
    window: Window,
}

impl Context {
    fn draw(&mut self) {
        if let Ok(swapchain_image) = self.swapchain.acquire_next_image() {
            self.window.pre_present_notify();

            let mut render_graph = RenderGraph::new();
            let swapchain_image = render_graph.bind_node(swapchain_image);

            // Rendering goes here!
            render_graph.clear_color_image_value(swapchain_image, [1.0, 0.0, 1.0]);
            let _ = self.device;

            let swapchain_image = self
                .display
                .resolve_image(render_graph, swapchain_image)
                .unwrap();
            self.swapchain.present_image(swapchain_image, 0, 0);
        }
    }
}
