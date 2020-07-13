#![deny(warnings)]
#![allow(dead_code)]

extern crate pretty_env_logger;

#[macro_use]
extern crate log as log_crate;

pub mod camera;
pub mod color;
pub mod config;
pub mod fx;
pub mod game;
pub mod gpu;
pub mod input;
pub mod math;
pub mod pak;

/// Things, particularly traits, which are used in almost every single Screen 13 game.
pub mod prelude {
    pub use {
        super::{math::Extent, DynScreen, Engine, Gpu, Input, Program, Render, Screen},
        log_crate::{debug, error, info, trace, warn},
    };
}

pub(crate) mod private {
    pub trait Sealed {}
}

mod error;
mod program;

// TODO: Remove Error from pub
pub use self::{
    error::Error,
    gpu::{Gpu, Render},
    input::Input,
    program::Program,
};

use {
    self::{config::Config, game::Game},
    std::{
        collections::VecDeque,
        ops::Add,
        time::{Duration, Instant},
    },
    winit::{
        event::{Event, VirtualKeyCode, WindowEvent},
        event_loop::ControlFlow,
    },
};

#[cfg(debug_assertions)]
use num_format::{Locale, ToFormattedString};

const NOMINAL_FRAME_MICROS: u64 = 15_000; // TODO: Kill with fire - Need to sync up with latest Winit patterns and get rid of this!!!!

pub type DynScreen = Box<dyn Screen>;

/// Only required when you are not running an engine instance but still using other
/// engine types and you want debugging setup.
pub fn init_debug() {
    pretty_env_logger::init();

    /*
    TODO
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Debug).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();*/

    info!("Screen 13 v0.1.0");
}

pub struct Engine {
    config: Config,
    game: Game,
}

impl Engine {
    pub fn new(program: Program) -> Self {
        #[cfg(debug_assertions)]
        init_debug();

        // Read the config file
        let config = Config::read(program.name).expect("Could not read engine config file");

        let game = if config.fullscreen() {
            Game::fullscreen(program.window_title, config.swapchain_len())
        } else {
            Game::windowed(
                program.window_title,
                config.swapchain_len(),
                config.window_dimensions(),
            )
        };

        Self { config, game }
    }

    pub fn gpu(&self) -> &Gpu {
        self.game.gpu()
    }

    pub fn run(self, screen: DynScreen) -> ! {
        let mut input = Input::default();
        let mut render_buf = VecDeque::with_capacity(self.config.render_buf_len());

        // This is the initial scene
        let mut screen: Option<DynScreen> = Some(screen);

        // Event loop state variables
        let mut last_frame = Instant::now();
        #[cfg(debug_assertions)]
        let mut started = last_frame;
        let mut redraw = false;

        let config = self.config;

        // Pump events until the application exits
        self.game.run(move |event, game, control_flow| {
            *control_flow =
                ControlFlow::WaitUntil(last_frame.add(Duration::from_micros(NOMINAL_FRAME_MICROS)));
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input: keyboard_input,
                        ..
                    } => {
                        if let Some(keycode) = keyboard_input.virtual_keycode {
                            match keycode {
                                VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
                                VirtualKeyCode::F11 => todo!("Toggle fullscreen functionality"),
                                _ => (),
                            };
                        }

                        input.keys.handle(&keyboard_input);
                    }
                    WindowEvent::Resized(dims) => game.resize(dims.into()),
                    _ => {}
                },
                Event::RedrawEventsCleared => game.request_redraw(),
                Event::RedrawRequested(_) => redraw = true,
                _ => {}
            }

            let now = Instant::now();

            if *control_flow == ControlFlow::Exit {
                return;
            } else if !redraw || now < last_frame.add(Duration::from_micros(NOMINAL_FRAME_MICROS)) {
                *control_flow = ControlFlow::WaitUntil(
                    last_frame.add(Duration::from_micros(NOMINAL_FRAME_MICROS)),
                );
                return;
            } else {
                redraw = false
            }

            // Keep the rendering buffer from overflowing
            while render_buf.len() >= config.render_buf_len() {
                render_buf.pop_back();
            }

            // Render & present the screen, saving the result in our buffer
            let render = screen.as_ref().unwrap().render(game.gpu());
            if let Some(frame) = game.present(render) {
                render_buf.push_front(frame);
            }

            // Update the current scene state, potentially returning a new one
            screen = Some(screen.take().unwrap().update(game.gpu(), &input));

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

            last_frame = now;
        });
    }
}

/// Screen provides the ability to render using the given
/// GPU and optionally to provide a new Screen result.
pub trait Screen {
    /// TODO
    fn render(&self, gpu: &Gpu) -> Render;

    /// TODO
    fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen;
}
