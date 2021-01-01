//#![deny(warnings)]
#![allow(dead_code)]

#[macro_use]
extern crate log;

pub mod camera;
pub mod color;
pub mod config;
pub mod fx;
pub mod gpu;
pub mod input;
pub mod math;

/// Note about keys: When baking assets using the .toml format you will not need to use the .toml
/// extension in order to load and use the assets at runtime. For instance, when trying to read a
/// model packed at `models/thing.toml` you might: `gpu.read_model("models/thing")`
pub mod pak;

/// Things, particularly traits, which are used in almost every single Screen 13 program.
pub mod prelude {
    pub use super::{DynScreen, Engine, Gpu, Input, Pool, Program, Render, Screen};
}

/// Like prelude, but everything
pub mod prelude_all {
    pub use super::{
        camera::*, color::*, config::*, fx::*, gpu::Material, gpu::*, input::*, math::*,
        pak::Material as PakMaterial, pak::*, prelude::*, program::*,
    };
}

pub(crate) mod error;
mod program;

pub use self::{
    color::{AlphaColor, Color},
    gpu::{Gpu, Pool, Render},
    input::Input,
    program::Program,
};

use {
    self::{config::Config, gpu::Swapchain, math::Extent},
    crate::gpu::Op,
    std::{cmp::Ordering, collections::VecDeque, convert::TryFrom},
    winit::{
        dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        monitor::VideoMode,
        window::{Fullscreen, Icon, Window, WindowBuilder},
    },
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

pub type DynScreen = Box<dyn Screen>;

const MINIMUM_WINDOW_SIZE: usize = 420;
const RENDER_BUF_LEN: usize = 3;

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

pub struct Engine {
    config: Config,
    event_loop: Option<EventLoop<()>>,
    dims: Extent,
    gpu: Gpu,
    swapchain: Swapchain,
    window: Window,
}

impl Engine {
    pub fn new<'a, 'b, P: AsRef<Program<'a, 'b>>>(program: P) -> Self {
        let program = program.as_ref();
        let config = Config::read(program.name, program.author).unwrap();
        let fullscreen = config.fullscreen().unwrap_or(program.fullscreen);

        if fullscreen {
            Self::new_fullscreen(program, config)
        } else {
            Self::new_window(program, config)
        }
    }

    fn new_builder(
        program: &Program,
        config: Config,
        event_loop: EventLoop<()>,
        builder: WindowBuilder,
        dims: Extent,
    ) -> Self {
        let icon = program
            .icon
            .as_ref()
            .map(|icon| Icon::try_from(icon).unwrap());
        let window = builder
            .with_resizable(program.resizable)
            .with_title(program.title)
            .with_window_icon(icon)
            .build(&event_loop)
            .unwrap();
        let (gpu, driver, surface) = Gpu::new(&window);
        let swapchain = Swapchain::new(&driver, surface, dims, config.swapchain_len());

        Self {
            config,
            dims,
            event_loop: Some(event_loop),
            gpu,
            swapchain,
            window,
        }
    }

    fn new_fullscreen(program: &Program, config: Config) -> Self {
        let mut builder = WindowBuilder::new();
        let event_loop = EventLoop::new();
        let primary_monitor = event_loop.primary_monitor().unwrap();

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

        Self::new_builder(program, config, event_loop, builder, dims)
    }

    fn new_window(program: &Program, config: Config) -> Self {
        let dims = config.window_dimensions();
        let mut builder = WindowBuilder::new();
        let event_loop = EventLoop::new();

        #[cfg(debug_assertions)]
        debug!("Building {}x{} window", dims.x, dims.y);

        // Setup windowed mode
        let physical_dims: PhysicalSize<_> = dims.into();
        builder = builder
            .with_fullscreen(None)
            .with_inner_size(physical_dims)
            .with_min_inner_size(LogicalSize::new(
                MINIMUM_WINDOW_SIZE as f32,
                MINIMUM_WINDOW_SIZE as f32,
            ));

        let res = Self::new_builder(program, config, event_loop, builder, dims);

        // In windowed mode set the screen position to be nicely centered
        if let Some(monitor) = res.window.current_monitor() {
            let (half_monitor_width, half_monitor_height) =
                (monitor.size().width >> 1, monitor.size().height >> 1);
            let (half_window_width, half_window_height) = (dims.x >> 1, dims.y >> 1);
            let window_x = half_monitor_width - half_window_width;
            let window_y = half_monitor_height - half_window_height;
            res.window
                .set_outer_position(PhysicalPosition::new(window_x, window_y));
        }

        res
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    fn present(&mut self, frame: Render) -> Vec<Box<dyn Op>> {
        let (mut target, ops) = frame.resolve();

        // We work-around this condition, below, but it is not expected that a well-formed program would ever do this
        debug_assert!(!ops.is_empty());

        // If the render had no operations performed on it then it is uninitialized and we don't need to do anything with it
        if !ops.is_empty() {
            // Target can be dropped directly after presentation, it will return to the pool. If for some reason the pool
            // is drained before the hardware is finished with target the underlying texture is still referenced by the operations.
            self.swapchain.present(&mut target);
        }

        ops
    }

    pub fn run(mut self, screen: DynScreen) -> ! {
        let mut input = Input::default();
        let mut render_buf = VecDeque::with_capacity(RENDER_BUF_LEN);

        // This is the initial scene
        let mut screen: Option<DynScreen> = Some(screen);

        #[cfg(debug_assertions)]
        info!("Starting event loop");

        #[cfg(debug_assertions)]
        let mut started = Instant::now();

        let event_loop = self.event_loop.take().unwrap();

        // Pump events until the application exits
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input: keyboard_input,
                        ..
                    } => input.keys.handle(&keyboard_input),
                    WindowEvent::Resized(dims) => self.dims = dims.into(),
                    _ => {}
                },
                Event::RedrawEventsCleared => self.window.request_redraw(),
                Event::MainEventsCleared | Event::RedrawRequested(_) => {
                    // Keep the rendering buffer from overflowing
                    while render_buf.len() >= RENDER_BUF_LEN {
                        render_buf.pop_back();
                    }

                    // Render & present the screen, saving the result in our buffer
                    let render = screen.as_ref().unwrap().render(&self.gpu, self.dims);
                    render_buf.push_front(self.present(render));

                    // Update the current scene state, potentially returning a new one
                    screen = Some(screen.take().unwrap().update(&self.gpu, &input));

                    // We have handled all input
                    input.keys.clear();

                    #[cfg(debug_assertions)]
                    {
                        let now = Instant::now();
                        let elapsed = now - started;
                        started = now;
                        let fps = (1_000_000_000.0 / elapsed.as_nanos() as f64) as usize;
                        match fps {
                            fps if fps >= 59 => debug!(
                                "Frame complete: {}ns ({}fps buf={})",
                                elapsed.as_nanos().to_formatted_string(&Locale::en),
                                fps.to_formatted_string(&Locale::en),
                                render_buf.len()
                            ),
                            fps if fps >= 50 => info!(
                                "Frame complete: {}ns ({}fps buf={}) (FRAME DROPPED)",
                                elapsed.as_nanos().to_formatted_string(&Locale::en),
                                fps.to_formatted_string(&Locale::en),
                                render_buf.len()
                            ),
                            _ => warn!(
                                "Frame complete: {}ns ({}fps buf={}) (STALLED)",
                                elapsed.as_nanos().to_formatted_string(&Locale::en),
                                fps.to_formatted_string(&Locale::en),
                                render_buf.len()
                            ),
                        }
                    }
                }
                _ => {}
            }
        });
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Program::default())
    }
}

/// The result of presenting a render to the screen. Hold this around for a few frames to
/// give the GPU time to finish processing it.
pub struct Frame(Vec<Box<dyn Op>>);

/// Screen provides the ability to render using the given
/// GPU and optionally to provide a new Screen result.
pub trait Screen {
    /// TODO
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render;

    /// TODO
    fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen;
}
