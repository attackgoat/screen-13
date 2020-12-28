//#![deny(warnings)]
#![allow(dead_code)]

#[macro_use]
extern crate log;

pub mod camera;
pub mod color;
pub mod config;
pub mod fx;
pub mod game;
pub mod gpu;
pub mod input;
pub mod math;

/// Note about keys: When baking assets using the .toml format you will not need to use the .toml extension in order to load and
/// use the assets at runtime. For instance, when trying to read a model packed at `models/thing.toml` you might: `gpu.load_model("models/thing")`
pub mod pak;

/// Things, particularly traits, which are used in almost every single Screen 13 program.
pub mod prelude {
    pub use super::{
        color::CORNFLOWER_BLUE, math::Extent, math::*, DynScreen, Engine, Gpu, Input, Pool,
        Program, Render, Screen,
    };
}

mod error;
mod program;

// TODO: Remove Error from pub
pub use self::{
    color::{AlphaColor, Color},
    error::Error,
    gpu::{Gpu, Pool, Render},
    input::Input,
    program::Program,
};

use {
    self::{config::Config, game::Game, math::Extent},
    std::collections::VecDeque,
    winit::{
        event::{Event, VirtualKeyCode, WindowEvent},
        event_loop::ControlFlow,
    },
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

pub type DynScreen = Box<dyn Screen>;

pub struct Engine {
    config: Config,
    game: Game,
}

impl Engine {
    pub fn new(program: Program) -> Self {
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
        #[cfg(debug_assertions)]
        let mut started = Instant::now();

        let config = self.config;

        // Pump events until the application exits
        self.game.run(move |event, game, control_flow| {
            *control_flow = ControlFlow::Wait;
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
                Event::MainEventsCleared | Event::RedrawRequested(_) => {
                    // Keep the rendering buffer from overflowing
                    while render_buf.len() >= config.render_buf_len() {
                        render_buf.pop_back();
                    }

                    // Render & present the screen, saving the result in our buffer
                    let render = screen.as_ref().unwrap().render(game.gpu(), game.dims());
                    render_buf.push_front(game.present(render));

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
                }
                _ => {}
            }
        });
    }
}

/// Screen provides the ability to render using the given
/// GPU and optionally to provide a new Screen result.
pub trait Screen {
    /// TODO
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render;

    /// TODO
    fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen;
}
