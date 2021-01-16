//! _Screen 13_ is an easy-to-use 2D/3D rendering engine in the spirit of
//! [QBasic](https://en.wikipedia.org/wiki/QBasic).
//!
//! _Screen 13_ supports DirectX 11/12, Metal, Vulkan and OpenGL (ES3, WebGL2).
//!
//! This crate is intended to be integrated into other libraries and programs which require very
//! high performance graphics code, but which do not want to know _anything_ about the underlying
//! graphics hardware or programming interfaces.
//!
//! Before starting you should be familar with these topics:
//! - The Rust Programming Language
//!   ([_beginner level_](https://doc.rust-lang.org/book/ch01-02-hello-world.html))
//! - Pixel formats such as 24bpp RGB
//!   ([_optional_](https://en.wikipedia.org/wiki/Color_depth#True_color_(24-bit)))
//! - Vertex formats such as [POSITION, TEXCOORD0]
//!   ([_optional_](https://www.khronos.org/opengl/wiki/Vertex_Specification#Theory))
//! - Common file formats (`.gltf`, `.png`, _etc.._)
//! - _Some notion about what a GPU might be_
//!
//! With almost striking exception, which appear in **_NOTE:_** sections only, no further graphics
//! API-specific concepts need to be introduced in order to master _Screen 13_ and implement
//! exceptionally fast graphics code.
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
//! Next, pick a shared reference type. We'll use [`std::rc::Rc`]:
//!
//! ```
//! use screen_13::prelude_rc::*;
//! ```
//!
//! Then, for a console program:
//!
//! ```
//! /// Creates a 128x128 pixel jpeg file as `output.jpg`.
//! fn main() {
//!     let gpu = Gpu::offscreen();
//!     let mut image = gpu.render((128u32, 128u32));
//!     image.clear().record();
//!     image.encode().record("output.jpg");
//! }
//! ```
//!
//! Or, for a windowed program:
//!
//! ```
//! /// Paints a magenta window at 60 glorious frames per second.
//! fn main() {
//!     let engine = Engine::default();
//!     engine.run(Box::new(Foo))
//! }
//!
//! struct Foo;
//!
//! impl Screen<RcK> for Foo {
//!     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
//!         let mut frame = gpu.render(dims);
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
//! ## Screen 13 Concepts
//!
//! _Screen 13_ offers two general modes of operation, both of which focus on the [`Gpu`] type:
//! - [`Gpu::offscreen()`]: _For headless rendering, such as from a console program_
//! - The [`Screen`] trait: _Provides a fullscreen graphics mode or paints a window_
//!
//! ## Shared References
//!
//! A [`Gpu`]'s resources, such as bitmaps and models, use either [`std::sync::Arc`] or
//! [`std::rc::Rc`] types to track their internal states.
//!
//! _Screen 13_ offers the following preludes to easily specifiy the shared reference type:
//!
//! | Scenario                    | Recommended `use`            | _Screen 13_ Types               |
//! |-----------------------------|------------------------------|---------------------------------|
//! | Single-Threaded Program     | `screen_13::prelude_rc::*;`  | `Gpu`, `Render`, _etc..._       |
//! | Multi-Threaded Program      | `screen_13::prelude_arc::*;` | `Gpu`, `Render`, _etc..._       |
//! | Both _or_ Choose at Runtime | `screen_13::prelude_all::*;` | `Gpu<P>`, `Render<P>`, _etc..._ |
//!
//! **_NOTE:_** The generic types require [`ptr::ArcK`] or [`ptr::RcK`] type parameters.
//!
//! ## `.pak` File Format
//!
//! Although data may be loaded at _runtime_, the highest performance can be achieved by pre-baking
//! data at _design-time_ and simply reading it at _runtime_.
//!
//! It is recommended to use the `.pak` format, _which includes optional *10:1-typical
//! compression*_, whenever possible. See the main
//! [README](https://github.com/attackgoat/screen-13) for more on this philosphy and the module
//! level documentation for more details on how to use this system with existing files and assets.

#![allow(dead_code)]
#![allow(clippy::needless_doctest_main)] // <-- The doc code is *intends* to show the whole shebang
//#![deny(warnings)]
#![warn(missing_docs)]
//#![warn(clippy::pedantic)]

// NOTE: If you are getting an error with the following line it is because both the `real-gfx` and
// `test-gfx` features are enabled at the same time.
#[cfg(feature = "mock-gfx")]
extern crate gfx_mock as gfx_impl;

#[macro_use]
extern crate log;

pub mod camera;
pub mod color;
pub mod fx;
pub mod gpu;
pub mod input;
pub mod math;
pub mod pak;

/// Things, particularly traits, which are used in almost every single _Screen 13_ program.
pub mod prelude {
    pub use super::{
        gpu::{Cache, Gpu, Render},
        input::Input,
        program::Program,
        ptr::{ArcK, RcK, Shared},
        DynScreen, Engine, Screen,
    };
}

/// Like [`prelude`], but contains all public exports.
///
/// Use this module for access to all _Screen 13_ resources from either [`std::sync::Arc`] or
/// [`std::rc::Rc`]-backed [`Gpu`] instances.
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

/// Like [`prelude_all`], but specialized for [`std::sync::Arc`]-backed [`Gpu`] instances.
///
/// Use this module if rendering will be done from multiple threads. See the main documentation for
/// each alias for more information.
pub mod prelude_arc {
    pub use super::prelude_all::*;

    /// Helpful type alias of `gpu::Bitmap<ArcK>`; see module documentation.
    pub type Bitmap = super::gpu::Bitmap<ArcK>;

    /// Helpful type alias of `gpu::Cache<ArcK>`; see module documentation.
    pub type Cache = super::gpu::Cache<ArcK>;

    /// Helpful type alias of `gpu::draw::Draw<ArcK>`; see module documentation.
    pub type Draw = super::gpu::draw::Draw<ArcK>;

    /// Helpful type alias of `DynScreen<ArcK>`; see module documentation.
    pub type DynScreen = super::DynScreen<ArcK>;

    /// Helpful type alias of `Engine<ArcK>`; see module documentation.
    pub type Engine = super::Engine<ArcK>;

    /// Helpful type alias of `fx::Fade<ArcK>`; see module documentation.
    #[cfg(feature = "blend-modes")]
    pub type Fade = super::fx::Fade<ArcK>;

    /// Helpful type alias of `gpu::text::Font<ArcK>`; see module documentation.
    pub type Font = super::gpu::text::Font<ArcK>;

    /// Helpful type alias of `gpu::Gpu<ArcK>`; see module documentation.
    pub type Gpu = super::gpu::Gpu<ArcK>;

    /// Helpful type alias of `gpu::draw::Material<ArcK>`; see module documentation.
    pub type Material = super::gpu::draw::Material<ArcK>;

    /// Helpful type alias of `gpu::draw::Mesh<ArcK>`; see module documentation.
    pub type Mesh = super::gpu::draw::Mesh<ArcK>;

    /// Helpful type alias of `gpu::Model<ArcK>`; see module documentation.
    pub type Model = super::gpu::Model<ArcK>;

    /// Helpful type alias of `gpu::draw::ModelCommand<ArcK>`; see module documentation.
    pub type ModelCommand = super::gpu::draw::ModelCommand<ArcK>;

    /// Helpful type alias of `gpu::Render<ArcK>`; see module documentation.
    pub type Render = super::gpu::Render<ArcK>;

    /// Helpful type alias of `ptr::Shared<ArcK>`; see module documentation.
    pub type Shared<T> = super::ptr::Shared<T, ArcK>;

    /// Helpful type alias of `gpu::draw::Skydome<ArcK>`; see module documentation.
    pub type Skydome = super::gpu::draw::Skydome<ArcK>;
}

/// Like [`prelude_all`], but specialized for [`std::rc::Rc`]-backed [`Gpu`] instances.
///
/// Use this module if rendering will be done from one thread only. See the main documentation for
/// each alias for more information.
pub mod prelude_rc {
    pub use super::prelude_all::*;

    /// Helpful type alias of `gpu::Bitmap<RcK>`; see module documentation.
    pub type Bitmap = super::gpu::Bitmap<RcK>;

    /// Helpful type alias of `gpu::Cache<RcK>`; see module documentation.
    pub type Cache = super::gpu::Cache<RcK>;

    /// Helpful type alias of `gpu::draw::Draw<RcK>`; see module documentation.
    pub type Draw = super::gpu::draw::Draw<RcK>;

    /// Helpful type alias of `DynScreen<RcK>`; see module documentation.
    pub type DynScreen = super::DynScreen<RcK>;

    /// Helpful type alias of `Engine<RcK>`; see module documentation.
    pub type Engine = super::Engine<RcK>;

    /// Helpful type alias of `fx::Fade<RcK>`; see module documentation.
    #[cfg(feature = "blend-modes")]
    pub type Fade = super::fx::Fade<RcK>;

    /// Helpful type alias of `gpu::text::Font<RcK>`; see module documentation.
    pub type Font = super::gpu::text::Font<RcK>;

    /// Helpful type alias of `gpu::Gpu<RcK>`; see module documentation.
    pub type Gpu = super::gpu::Gpu<RcK>;

    /// Helpful type alias of `gpu::draw::Material<RcK>`; see module documentation.
    pub type Material = super::gpu::draw::Material<RcK>;

    /// Helpful type alias of `gpu::draw::Mesh<RcK>`; see module documentation.
    pub type Mesh = super::gpu::draw::Mesh<RcK>;

    /// Helpful type alias of `gpu::Model<RcK>`; see module documentation.
    pub type Model = super::gpu::Model<RcK>;

    /// Helpful type alias of `gpu::draw::ModelCommand<RcK>`; see module documentation.
    pub type ModelCommand = super::gpu::draw::ModelCommand<RcK>;

    /// Helpful type alias of `gpu::Render<RcK>`; see module documentation.
    pub type Render = super::gpu::Render<RcK>;

    /// Helpful type alias of `ptr::Shared<RcK>`; see module documentation.
    pub type Shared<T> = super::ptr::Shared<T, RcK>;

    /// Helpful type alias of `gpu::draw::Skydome<RcK>`; see module documentation.
    pub type Skydome = super::gpu::draw::Skydome<RcK>;
}

/// Shared reference (`Arc` and `Rc`) implementation based on
/// [_archery_](https://crates.io/crates/archery).
pub mod ptr {
    pub use a_r_c_h_e_r_y::{ArcK, RcK};

    use {
        a_r_c_h_e_r_y::{SharedPointer, SharedPointerKind},
        std::ops::Deref,
    };

    /// A shared reference wrapper type, based on either [`std::sync::Arc`] or [`std::rc::Rc`].
    #[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct Shared<T, P>(SharedPointer<T, P>)
    where
        P: SharedPointerKind;

    impl<T, P> Shared<T, P>
    where
        P: SharedPointerKind,
    {
        pub(crate) fn new(val: T) -> Self {
            Self(SharedPointer::new(val))
        }

        /// Returns a constant pointer to the value.
        pub fn as_ptr(shared: &Self) -> *const T {
            //SharedPointer::as_ptr(&shared.0)
            SharedPointer::as_ptr(&shared.0)
        }

        /// Returns a copy of the value.
        #[allow(clippy::should_implement_trait)]
        pub fn clone(shared: &Self) -> Self {
            shared.clone()
        }

        /// Returns `true` if two `Shared` instances point to the same underlying memory.
        pub fn ptr_eq(lhs: &Self, rhs: &Self) -> bool {
            SharedPointer::ptr_eq(&lhs.0, &rhs.0)
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

    impl<T, P> Default for Shared<T, P>
    where
        P: SharedPointerKind,
        T: Default,
    {
        fn default() -> Self {
            Self::new(Default::default())
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
}

mod config;
mod program;

pub use self::program::{Icon, Program};

use {
    self::{
        config::Config,
        gpu::{Gpu, Op, Render, Swapchain},
        input::Input,
        math::Extent,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    app_dirs::{get_app_root, AppDataType, AppDirsError, AppInfo},
    std::{
        cmp::Ordering,
        collections::VecDeque,
        convert::TryFrom,
        io::{Error, ErrorKind},
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

#[cfg(feature = "multi-monitor")]
use self::math::Area;

/// Helpful alias of `Box<dyn Screen>`; can be used to hold an instance of any `Screen`.
pub type DynScreen<P> = Box<dyn Screen<P>>;

/// Alias of either [`Render`] _or_ [`Vec<Option<Render>>`], used by [`Screen::render()`].
///
/// The output type depends on the value of the `multi-monitor` pacakge feature.
///
/// **_NOTE:_** This documentation was generated _without_ the `multi-monitor` feature.
#[cfg(not(feature = "multi-monitor"))]
pub type RenderReturn<P> = Render<P>;

/// Alias of either [`Render`] _or_ [`Vec<Option<Render>>`], used by [`Screen::render()`].
///
/// The output type depends on the value of the `multi-monitor` pacakge feature.
///
/// **_NOTE:_** This documentation was generated _with_ the `multi-monitor` feature.
#[cfg(feature = "multi-monitor")]
pub type RenderReturn<P> = Vec<Option<Render<P>>>;

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

        // We work-around this condition, below, but it is not expected that a well-formed program
        // would ever do this
        debug_assert!(!ops.is_empty());

        // If the render had no operations performed on it then it is uninitialized and we don't
        // need to do anything with it
        if !ops.is_empty() {
            // Target can be dropped directly after presentation, it will return to the pool. If for
            // some reason the pool is drained before the hardware is finished with target the
            // underlying texture is still referenced by the operations.
            self.swapchain.present(&mut target);
        }

        ops
    }

    /// Runs a program starting with the given `DynScreen`.
    ///
    /// Immediately after this call, `render` will be called on the screen, followed by `update`, ad
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
/// **_NOTE:_** See the [`fx`] module for some pre-built examples of such screen ownership
/// structures.
///
/// While a program event loop is running the `Screen` functions are called repeatedly in this
/// order:
/// 1. `render`: _Provide a `Render` instance in which rendering operations have been recorded_
/// 2. `update`: _Respond to window input and either return `self` (no change) or a new `DynScreen`_
///
/// ## Implementing `Screen`
///
/// Implementors of `Screen` invariably need to access resources loaded or read from the `Gpu`,
/// such as bitmaps and models. To accomplish resource access you might either offer a loading
/// function or perform the needed loads at runtime, using `RefCell` to gain interior mutability
/// during the `render` call.
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
    /// **_NOTE:_** It is considered undefined behavior to return a render which has not recorded
    /// any commands, as shown:
    ///
    /// ```
    /// fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///     // This is UB because the graphics hardware might have been using this render to store
    ///     // an 8K atlas of 😸's, and it is not guaranteed to be initialized.
    ///     // Hey, the more you know!
    ///     gpu.render(dims)
    /// }
    /// ```
    ///
    /// ## Multiple Monitors
    ///
    /// Support for multiple monitors is an advanced feature which must be enabled manually. To
    /// enable multiple monitor support, add the `multi-monitor` feature to the _Screen 13_
    /// dependency in your `Cargo.toml`.
    ///
    /// **_NOTE:_** The automatically generated documentation shows the default function signature.
    /// When in multiple monitor mode, you may want to run `cargo doc --features "multi-monitor"` in
    /// order to see the correct signature.
    ///
    /// Summary of multiple monitor mode differences:
    /// - The `dims: Extent` parameter becomes `viewports: &[Area]`
    /// - The `Render` return type becomes `Vec<Option<Render>>`
    /// - Each returned `Render` corresponds to the viewport of the same index
    /// - Return `Some` for viewports which should be painted
    ///
    /// When in window mode, the `viewports` slice length is `1`. Only one operating system window
    /// is opened.
    fn render(
        &self,
        gpu: &Gpu<P>,
        #[cfg(not(feature = "multi-monitor"))] dims: Extent,
        #[cfg(feature = "multi-monitor")] viewports: &[Area],
    ) -> RenderReturn<P>;

    /// Responds to user input and either provides a new `DynScreen` instance or `self` to indicate
    /// no-change.
    ///
    /// After `update`, `render` will be called on the returned screen, and the previous screen will
    /// be dropped.
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
