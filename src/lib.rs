//! An easy-to-use 2D/3D rendering engine in the spirit of QBasic.
//!
//! This crate provides image rendering types and functions. It is intended to be integrated into
//! other libraries and programs which require very high performance graphics code, but which do
//! not want to know _anything_ about the underlying graphics hardware or programming interfaces.
//!
//! Before starting you should be familar with these topics:
//! - The Rust Programming Language
//!   ([_beginner level_](https://doc.rust-lang.org/book/ch01-02-hello-world.html))
//! - Common file formats (`.gltf`, `.png`, _etc.._)
//! - Pixel formats such as 24bpp RGB
//!   ([_optional_](https://en.wikipedia.org/wiki/Color_depth#True_color_(24-bit)))
//! - Vertex formats such as [POSITION, TEXCOORD0]
//!   ([_optional_](https://www.khronos.org/opengl/wiki/Vertex_Specification#Theory))
//! - _Some notion about what a GPU might be_
//!
//! With almost striking exception, which appear in "_NOTE:_" sections only, no further graphics
//! API-specific concepts need to be introduced in order to master Screen 13 and implement
//! exceptionally fast graphics code.
//!
//! _TL;DR:_ Screen 13 adds state-of-the-art Vulkan/Metal/DirectX/GL to your code, easily.
//!
//! # Usage
//!
//! First, add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! screen_13 = "0.1"
//! ```
//!
//! Next, for a console program:
//!
//! ```
//! /// Creates a 128x128 pixel jpeg file as `output.jpg`.
//! fn main() {
//!     let gpu = screen_13::Gpu::offscreen();
//!     let render = gpu.render((128u32, 128u32));
//!     render.clear().record();
//!     render.encode().record("output.jpg");
//! }
//! ```
//!
//! Or, for a windowed program:
//!
//! ```
//! use screen_13::prelude_all::*;
//!
//! /// Paints a magenta window at 60 glorious frames per second.
//! fn main() {
//!     let engine = Engine::default();
//!     engine.run(Box::new(FooProgram))
//! }
//!
//! struct FooProgram;
//!
//! impl Screen for FooProgram {
//!     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
//!         let frame = gpu.render(dims);
//!         frame.clear().with(MAGENTA).record(); // <-- 🔥
//!         frame
//!     }
//!
//!     fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
//!         // Never exits
//!         self
//!     }
//! }
//! ```
//!
//! # Screen 13 Concepts
//!
//! Screen 13 offers libraries and applications two general modes of operation, both of which focus
//! on the `Gpu` type:
//! - `Gpu::offscreen()`:  For headless rendering, such as from a console program
//! - The `Screen` trait: Provides a fullscreen graphics mode or paints a window
//!
//! _NOTE_: Resources loaded or read from a `Gpu` created in headless or screen modes cannot be
//! used with other instances, including of the same mode. This is a limitation only because the
//! code to share the resources properly has not be started yet.
//!
//! ## Screen 13 PAK Format
//!
//! Although data may be loaded at _runtime_, the highest performance can be achieved by pre-baking
//! data at _design-time_ and simply reading it at _runtime_.
//!
//! It is recommended to use the `.pak` file format, _which includes optional *10:1-typical
//! compression*_, whenever possible. See the main
//! [README](https://github.com/attackgoat/screen-13) for more on this philosphy and the module
//! level documentation for more details on how to use this system with existing files and assets.

#![allow(dead_code)]
#![allow(clippy::needless_doctest_main)] // <-- The doc code is *intends* to show the whole shebang
//#![deny(warnings)]
#![warn(missing_docs)]

#[macro_use]
extern crate log;

pub mod camera;
pub mod color;
pub mod fx;
pub mod gpu;
pub mod input;
pub mod math;
pub mod pak;

/// Things, particularly traits, which are used in almost every single Screen 13 program.
pub mod prelude {
    pub use super::{
        gpu::{Cache, Gpu, Render},
        input::Input,
        program::Program,
        ArcK, DynScreen, Engine, RcK, Screen,
    };
}

/// Like `[prelude]`, but contains all public exports.
///
/// Use this module for access to all Screen 13 resources from either `[Rc]` or `[Arc]`-backed
/// `[Gpu]` instances.
pub mod prelude_all {
    pub use super::{
        camera::*,
        color::*,
        fx::*,
        gpu::draw::*,
        gpu::text::*,
        gpu::vertex::*,
        gpu::write::*,
        gpu::*,
        input::*,
        math::*,
        pak::MaterialDesc,
        pak::{id::*, *},
        prelude::*,
        *,
    };
}

/// Like `[prelude_all]`, but specialized for `[Rc]`-backed `[Gpu]` instances.
///
/// Use this module if rendering will be done from one thread only. See the main documentation for
/// each alias for more information.
pub mod prelude_rc {
    pub use super::prelude_all::*;

    /// Helpful type alias of `gpu::draw::Draw::<RcK>`; see module documentation.
    pub type Draw = super::gpu::draw::Draw<RcK>;

    /// Helpful type alias of `DynScreen::<RcK>`; see module documentation.
    pub type DynScreen = super::DynScreen<RcK>;

    /// Helpful type alias of `Engine::<RcK>`; see module documentation.
    pub type Engine = super::Engine<RcK>;

    /// Helpful type alias of `gpu::text::Font::<RcK>`; see module documentation.
    pub type Font = super::gpu::text::Font<RcK>;

    /// Helpful type alias of `gpu::Gpu::<RcK>`; see module documentation.
    pub type Gpu = super::gpu::Gpu<RcK>;

    /// Helpful type alias of `gpu::Render::<RcK>`; see module documentation.
    pub type Render = super::gpu::Render<RcK>;
}

pub(crate) mod error;

mod config;
mod program;

pub use {
    self::program::{Icon, Program},
    archery::{ArcK, RcK},
};

use {
    self::{
        config::Config,
        gpu::{Gpu, Render, Swapchain},
        input::Input,
        math::Extent,
    },
    crate::gpu::Op,
    app_dirs::{get_app_root, AppDataType, AppDirsError, AppInfo},
    archery::{SharedPointer, SharedPointerKind},
    std::{
        cmp::Ordering,
        collections::VecDeque,
        convert::TryFrom,
        io::{Error, ErrorKind},
        ops::Deref,
        path::PathBuf,
    },
    winit::{
        dpi::{LogicalSize, PhysicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        monitor::VideoMode,
        window::{Fullscreen, Icon as WinitIcon, Window, WindowBuilder},
    },
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

/// Helpful alias of `Box<dyn Screen>`; can be used to hold an instance of any `Screen`.
pub type DynScreen<P> = Box<dyn Screen<P>>;

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

fn program_root(program: &Program) -> Result<PathBuf, Error> {
    root(program.name, program.author)
}

/// Gets the filesystem root for a given program name and author.
///
/// The returned path is a good place
/// to store program configuration and data on a per-user basis.
pub fn root(name: &'static str, author: &'static str) -> Result<PathBuf, Error> {
    // Converts the app_dirs crate AppDirsError to a regular IO Error
    match get_app_root(AppDataType::UserConfig, &AppInfo { name, author }) {
        Err(err) => Err(match err {
            AppDirsError::Io(err) => err,
            AppDirsError::InvalidAppInfo => Error::from(ErrorKind::InvalidInput),
            AppDirsError::NotSupported => Error::from(ErrorKind::InvalidData),
        }),
        Ok(res) => Ok(res),
    }
}

/// Pumps an operating system event loop in order to refresh a `Gpu`-created image at the refresh
/// rate of the monitor. Requires a `DynScreen` instance to render.
pub struct Engine<P>
where
    P: 'static + SharedPointerKind,
{
    config: Config,
    event_loop: Option<EventLoop<()>>,
    dims: Extent,
    gpu: Gpu<P>,
    swapchain: Swapchain,
    window: Window,
}

impl<P> Engine<P>
where
    P: SharedPointerKind,
{
    /// Constructs a new `Engine` from the given `Program` description.
    ///
    /// _NOTE:_ This function loads any existing user configuration file and may override program
    /// description options in order to preserve the user experience.
    ///
    /// ## Examples
    ///
    /// ```
    /// use screen_13::prelude_all::*;
    ///
    /// fn main() {
    ///     let foo = Program::new("UltraMega III", "Nintari, Inc.")
    ///                 .with_title("UltraMega III: Breath of Fire")
    ///                 .with_window();
    ///     let engine = Engine::new(foo); //    We ask for windowed mode, but we ...
    ///     engine.run(...)                // <- ... get fullscreen because of some previous run. 😂
    /// }
    /// ```
    pub fn new<'a, 'b, R: AsRef<Program<'a, 'b>>>(program: R) -> Self {
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
            .map(|icon| WinitIcon::try_from(icon).unwrap());
        let window = builder
            .with_resizable(program.resizable)
            .with_title(program.title)
            .with_window_icon(icon)
            .build(&event_loop)
            .unwrap();
        let (gpu, swapchain) = unsafe { Gpu::new(&window, dims, config.swapchain_len()) };

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
            // .with_visible(false)
            .with_min_inner_size(LogicalSize::new(
                MINIMUM_WINDOW_SIZE as f32,
                MINIMUM_WINDOW_SIZE as f32,
            ));

        Self::new_builder(program, config, event_loop, builder, dims)

        /* TODO: This is ugly on x11
        use winit::dpi::PhysicalPosition;

        // In windowed mode set the screen position to be nicely centered
        if let Some(monitor) = res.window.current_monitor() {
            let (half_monitor_width, half_monitor_height) =
                (monitor.size().width >> 1, monitor.size().height >> 1);
            let (half_window_width, half_window_height) = (dims.x >> 1, dims.y >> 1);
            let window_x = half_monitor_width - half_window_width;
            let window_y = half_monitor_height - half_window_height;
            res.window
                .set_outer_position(PhysicalPosition::new(window_x, window_y));
            // res.window.set_visible(true);
        }*/
    }

    /// Borrows the `Gpu` instance.
    pub fn gpu(&self) -> &Gpu<P> {
        &self.gpu
    }

    unsafe fn present(&mut self, frame: Render<P>) -> Vec<Box<dyn Op<P>>> {
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

    /// Runs a program starting with the given `DynScreen`.
    ///
    /// Immediately after this call, `draw` will be called on the screen, followed by `update`, ad
    /// infinium. This call does not return to the calling code.
    ///
    /// ## Examples
    ///
    /// ```
    /// use screen_13::prelude_all::*;
    ///
    /// fn main() {
    ///     let engine = Engine::default();
    ///     engine.run(Box::new(FooScreen)) // <- Note the return value which is the no-return bang
    ///                                     //    "value", inception. 🤯
    /// }
    ///
    /// struct FooScreen;
    ///
    /// impl Screen for FooScreen {
    ///     ...
    /// }
    /// ```
    pub fn run(mut self, screen: DynScreen<P>) -> ! {
        let mut input = Input::default();
        let mut render_buf = VecDeque::with_capacity(RENDER_BUF_LEN);

        // This is the initial scene
        let mut screen: Option<DynScreen<P>> = Some(screen);

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
                    render_buf.push_front(unsafe { self.present(render) });

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

impl<P> Default for Engine<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self::new(Program::default())
    }
}

impl<P> From<Program<'_, '_>> for Engine<P>
where
    P: SharedPointerKind,
{
    fn from(program: Program<'_, '_>) -> Self {
        Self::new(program)
    }
}

impl<P> From<&Program<'_, '_>> for Engine<P>
where
    P: SharedPointerKind,
{
    fn from(program: &Program<'_, '_>) -> Self {
        Self::new(program)
    }
}

/// A window-painting and user input handling type.
///
/// Types implementing `Screen` are able to present high-frequency images to the user and control
/// the flow of the program by switching out `Screen` implementations on the fly. Instances of
/// `Screen` are provided to `Engine` for normal use, but can also be owned in a parent-child
/// relationship to create sub-screens or to dynamically render.
///
/// _NOTE:_ See the `fx` module for some pre-built examples of such screen ownership structures.
///
/// While a program event loop is running the `Screen` functions are called repeatedly in this
/// order:
/// 1. `render`: Provide a `Render` instance in which rendering operations have been recorded.
/// 2. `update`: Respond to window input and either return `self` (no change) or a new `DynScreen`.
///
/// ## Implementing `Screen`
///
/// Implementors of `Screen` invariably need to access resources loaded or read from the `Gpu`,
/// such as bitmaps and models. To accomplish resource access you might either offer a loading
/// function or perform the needed loads at runtime, using `RefCell` to gain interior mutability
/// during the `render(...)` call.
///
/// Example load before `render`:
///
/// ```
/// impl FooScreen {
///     fn load(gpu: &Gpu, pak: &mut PakFile) -> Self {
///         Self {
///             bar: gpu.read_bitmap(gpu, &mut pak, "bar"),
///         }
///     }
/// }
/// ```
///
/// Example load during `render` (_`update` works too_):
///
/// ```
/// impl Screen for FooScreen {
///     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
///         *self.bar.borrow_mut() = Some(gpu.read_bitmap(gpu, self.pak.borrow_mut(), "bar"));
///         ...
///     }
///
///     ...
/// }
/// ```
pub trait Screen<P>
where
    P: 'static + SharedPointerKind,
{
    /// When paired with an `Engine`, generates images presented to the physical display adapter
    /// using a swapchain and fullscreen video mode or operating system window.
    ///
    /// ## Examples
    ///
    /// Calling `render` on another `Screen`:
    ///
    /// ```
    /// let foo: DynScreen = ...
    /// let gpu = Gpu::offscreen();
    ///
    /// // Ask foo to render a document
    /// let foo_doc = foo.render(&gpu, Extent::new(1024, 128));
    ///
    /// // 🤮 Ugh! I didn't like it!
    /// foo_doc.clear().record();
    ///
    /// println!("{:?}", foo_doc);
    /// ```
    ///
    /// Responding to `render` as a `Screen` implementation:
    ///
    /// ```
    /// fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///     let frame = gpu.render(dims);
    ///
    ///     // 🥇 It's some of my best work!
    ///     frame.clear().with(GREEN).record();
    ///
    ///     frame
    /// }
    /// ```
    ///
    /// _NOTE:_ It is considered undefined behavior to return a render which has not recorded any
    /// commands, as shown:
    ///
    /// ```
    /// fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///     // This is UB because the graphics hardware might have been using this render to store
    ///     // an 8K atlas of 😸's, and it is not guaranteed to be initialized.
    ///     // Hey, the more you know!
    ///     gpu.render(dims)
    /// }
    /// ```
    fn render(&self, gpu: &Gpu<P>, dims: Extent) -> Render<P>;

    /// Responds to user input and either provides a new `DynScreen` instance or `self` to indicate
    /// no-change. After `update(...)`, `render(...)` will be called on the returned screen, and
    /// the previous screen will be dropped.
    ///
    /// ## Examples
    ///
    /// Render this screen forever, never responding to user input or exiting:
    ///
    /// ```
    /// fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
    ///     // 🙈 Yolo!
    ///     self
    /// }
    /// ```
    ///
    /// A kind of three way junction. Goes to `BarScreen` when Home is pressed, otherwise
    /// presents the current screen, rendering for five seconds before quitting:
    ///
    /// ```
    /// fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
    ///     let wall_time = ...
    ///     if input.keys.is_key_down(Key::Home) {
    ///         Box::new(BarScreen)
    ///     } else if wall_time < 5.0 {
    ///         self
    ///     } else {
    ///         // 👋
    ///         exit(0);
    ///     }
    /// }
    /// ```
    fn update(self: Box<Self>, gpu: &Gpu<P>, input: &Input) -> DynScreen<P>;
}

#[derive(Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Shared<T, P>(SharedPointer<T, P>)
where
    P: SharedPointerKind;

impl<T, P> Shared<T, P>
where
    P: SharedPointerKind,
{
    pub fn clone(shared: &Self) -> Self {
        shared.clone()
    }
}

impl<T, P> Shared<T, P>
where
    P: SharedPointerKind,
{
    pub fn new(val: T) -> Self {
        Self(SharedPointer::new(val))
    }
}

impl<T, P> Clone for Shared<T, P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, P> Deref for Shared<T, P>
where
    P: SharedPointerKind,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
