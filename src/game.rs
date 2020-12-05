pub use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

use {
    crate::{
        gpu::{Driver, Gpu, Op, Render, Swapchain},
        math::Extent,
    },
    std::cmp::Ordering,
    winit::monitor::VideoMode,
};

const MINIMUM_WINDOW_SIZE: usize = 420;

fn area(size: PhysicalSize<u32>) -> u32 {
    size.height * size.width
}

fn bit_depth_24(video_mode: &VideoMode) -> bool {
    video_mode.bit_depth() == 24
}

fn cmp_area_and_refresh_rate(lhs: &VideoMode, rhs: &VideoMode) -> Ordering {
    // Sort the video modes by area first ...
    match area(lhs.size()).cmp(&area(rhs.size())) {
        area @ Ordering::Greater | area @ Ordering::Less => return area,
        _ => (),
    }

    // ... and when they're equal sort by refresh rate
    lhs.refresh_rate().cmp(&rhs.refresh_rate())
}

/// The result of presenting a render to the screen. Hold this around for a few frames to
/// give the GPU time to finish processing it.
pub struct Frame(Vec<Box<dyn Op>>);

pub struct Game {
    event_loop: Option<EventLoop<()>>,
    dims: Extent,
    gpu: Gpu,
    swapchain: Swapchain,
    window: Window,
}

impl Game {
    pub fn fullscreen(title: &str, swapchain_len: usize) -> Self {
        let mut builder = WindowBuilder::new().with_title(title);
        let event_loop = EventLoop::new();
        let primary_monitor = event_loop.primary_monitor().unwrap();
        // TODO: let dpi = primary_monitor.scale_factor();

        #[cfg(debug_assertions)]
        debug!("Building fullscreen window");

        // Sort all 24bit video modes from best to worst
        let mut video_modes = primary_monitor
            .video_modes()
            .filter(bit_depth_24)
            .collect::<Vec<_>>();
        video_modes.sort_by(cmp_area_and_refresh_rate);
        let best_video_mode = video_modes.pop().unwrap();
        let dims = best_video_mode.size().into();
        builder = builder.with_fullscreen(Some(Fullscreen::Exclusive(best_video_mode)));

        Self::new(event_loop, builder, dims, swapchain_len as u32)
    }

    fn new(
        event_loop: EventLoop<()>,
        builder: WindowBuilder,
        dims: Extent,
        swapchain_len: u32,
    ) -> Self {
        let window = builder.build(&event_loop).unwrap();
        let (gpu, surface) = Gpu::new(&window);
        let driver = Driver::clone(gpu.driver());
        let swapchain = Swapchain::new(driver, surface, dims, swapchain_len);

        Self {
            dims,
            event_loop: Some(event_loop),
            gpu,
            swapchain,
            window,
        }
    }

    pub fn windowed(title: &str, swapchain_len: usize, dims: Extent) -> Self {
        let event_loop = EventLoop::new();
        let mut builder = WindowBuilder::new().with_title(title);

        #[cfg(debug_assertions)]
        debug!("Building {}x{} window", dims.x, dims.y);

        // Setup fullscreen or windowed mode
        let physical_dims: PhysicalSize<_> = dims.into();
        builder = builder
            .with_inner_size(physical_dims) // TODO: Rename
            .with_min_inner_size(LogicalSize::new(
                MINIMUM_WINDOW_SIZE as f32,
                MINIMUM_WINDOW_SIZE as f32,
            ));

        /*/ In windowed mode set the screen position
        if let Some((width, height)) = dimensions {
        let monitor = get_primary_monitor();
        let (monitor_width, monitor_height) = monitor.get_dimensions();
        let (half_monitor_width, half_monitor_height) = (monitor_width >> 1, monitor_height >> 1);
        let (half_window_width, half_window_height) = (width >> 1, height);
        let window_x = half_monitor_width.max(half_window_width) - half_window_width;
        let window_y = half_monitor_height.max(half_window_height) - half_window_height;

        window.set_position(window_x as i32, window_y as i32);
        }*/

        Self::new(event_loop, builder, dims, swapchain_len as u32)
    }

    pub fn dims(&self) -> Extent {
        self.dims
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn present(&mut self, frame: Render) -> Frame {
        let (mut target, ops) = frame.resolve();

        // We work-around this condition, below, but it is not expected that a well-formed game would ever do this
        assert!(!ops.is_empty());

        // If the render had no operations performed on it then it is uninitialized and we don't need to do anything with it
        if !ops.is_empty() {
            // Target can be dropped directly after presentation, it will return to the pool. If for some reason the pool
            // is drained before the hardware is finished with target the underlying texture is still referenced by the operations.
            self.swapchain.present(&mut target);
        }

        Frame(ops)
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn resize(&mut self, dims: Extent) {
        self.dims = dims;
    }

    pub fn run<F>(mut self, mut event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, ()>, &mut Game, &mut ControlFlow),
    {
        let event_loop = self.event_loop.take().unwrap();
        let mut game = self;

        #[cfg(debug_assertions)]
        info!("Starting event loop");

        event_loop.run(move |event, _, control_flow| event_handler(event, &mut game, control_flow));
    }
}
