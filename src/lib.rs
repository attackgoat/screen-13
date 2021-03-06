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
//! ```rust
//! use screen_13::prelude_rc::*;
//! ```
//!
//! Then, for a console program:
//!
//! ```rust
//! # use screen_13::prelude_rc::*;
//! # fn __() {
//! /// Creates a 128x128 pixel jpeg file as `output.jpg`.
//! fn main() {
//!     let gpu = Gpu::offscreen();
//!     let mut image = gpu.render((128u32, 128u32));
//!     image.clear().record();
//!     image.encode().record("output.jpg");
//! }
//! # }
//! ```
//!
//! Or, for a windowed program:
//!
//! ```rust
//! # use screen_13::prelude_rc::*;
//! # fn __() {
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
//! # }
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

#![allow(dead_code)] // TODO: Remove at some point
#![allow(clippy::needless_doctest_main)] // <-- The doc code is *intends* to show the whole shebang
#![warn(missing_docs)]
//#![warn(clippy::pedantic)]

// Enable this only while debugging; remember, they're only warnings.... for us humans...
//#![deny(warnings)]

// NOTE: If you are getting an error with the following line it is because both the `impl-gfx` and
// `mock-gfx` features are enabled at the same time. Use "--no-default-features" to fix.
#[cfg(feature = "mock-gfx")]
extern crate gfx_backend_mock as gfx_impl;

// NOTE: If you are getting an error with the following line it is because both the `impl-gfx` and
// `test-gfx` features are enabled at the same time. Use "--no-default-features" to fix.
#[cfg(feature = "test-gfx")]
extern crate gfx_backend_test as gfx_impl;

#[macro_use]
extern crate log;

pub mod camera;
pub mod color;
pub mod fx;
pub mod gpu;
pub mod input;
pub mod math;
pub mod pak;

#[cfg(feature = "bake")]
pub mod bake;

/// Things, particularly traits, which are used in almost every single _Screen 13_ program.
pub mod prelude {
    pub use super::{
        gpu::{Cache, Gpu, Render},
        input::Input,
        ptr::{ArcK, RcK, Shared},
        DynScreen, Engine, Screen,
    };

    #[cfg(not(target_arch = "wasm32"))]
    pub use super::program::Program;
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

    /// Helpful type alias of `gpu::text::BitmapFont<ArcK>`; see module documentation.
    pub type BitmapFont = super::gpu::text::BitmapFont<ArcK>;

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

    /// Helpful type alias of `gpu::text::BitmapFont<RcK>`; see module documentation.
    pub type BitmapFont = super::gpu::text::BitmapFont<RcK>;

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
    pub use archery::{ArcK, RcK};

    use {
        archery::{SharedPointer, SharedPointerKind},
        std::ops::Deref,
    };

    /// A shared reference wrapper type, based on either [`std::sync::Arc`] or [`std::rc::Rc`].
    #[derive(Debug, Eq, Ord, PartialOrd)]
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

    impl<T, P> From<&Shared<T, P>> for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn from(val: &Self) -> Self {
            val.clone()
        }
    }

    impl<T, P> PartialEq for Shared<T, P>
    where
        P: SharedPointerKind,
    {
        fn eq(&self, other: &Self) -> bool {
            Self::ptr_eq(self, other)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod config;

#[cfg(not(target_arch = "wasm32"))]
mod program;

#[cfg(not(target_arch = "wasm32"))]
pub use self::program::{Icon, Program};

use {
    self::{
        gpu::{Gpu, Op, Render, Swapchain},
        input::Input,
        math::Extent,
    },
    archery::SharedPointerKind,
    directories::ProjectDirs,
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

#[cfg(not(target_arch = "wasm32"))]
use self::config::Config;

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

#[cfg(feature = "multi-monitor")]
use self::math::Area;

/// Helpful alias of `Box<dyn Screen>`; can be used to hold an instance of any `Screen`.
pub type DynScreen<P> = Box<dyn Screen<P>>;

const DEFAULT_RENDER_BUF_LEN: usize = 128;
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

#[cfg(not(target_arch = "wasm32"))]
fn program_root(program: &Program) -> Result<PathBuf, Error> {
    root(program.name, program.author)
}

/// Gets the filesystem root for a given program name and author.
///
/// The returned path is a good place to store program configuration and data on a per-user basis.
#[cfg(not(target_arch = "wasm32"))]
pub fn root(name: &'static str, author: &'static str) -> Result<PathBuf, Error> {
    // Converts the app_dirs crate AppDirsError to a regular IO Error
    match ProjectDirs::from("", author, name) {
        None => Err(Error::from(ErrorKind::InvalidInput)),
        Some(dirs) => Ok(dirs.config_dir().to_owned()),
    }
}

/// Pumps an operating system event loop in order to refresh a `Gpu`-created image at the refresh
/// rate of the monitor. Requires a `DynScreen` instance to render.
pub struct Engine<P>
where
    P: 'static + SharedPointerKind,
{
    #[cfg(not(target_arch = "wasm32"))]
    config: Config,

    event_loop: Option<EventLoop<()>>,
    gpu: Gpu<P>,

    #[cfg(debug_assertions)]
    started: Instant,

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
    /// # Examples
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # fn __() {
    /// let ultra_mega = Program::new("UltraMega III", "Nintari, Inc.")
    ///                   .with_title("UltraMega III: Breath of Fire")
    ///                   .with_window();
    /// let engine = Engine::new(ultra_mega);
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        let (gpu, swapchain) = unsafe { Gpu::new(&window, 3, true) };

        Self {
            event_loop: Some(event_loop),
            gpu,
            #[cfg(debug_assertions)]
            started: Instant::now(),
            swapchain,
            window,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_builder(
        program: &Program,
        config: Config,
        event_loop: EventLoop<()>,
        builder: WindowBuilder,
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
        let (gpu, swapchain) =
            unsafe { Gpu::new(&window, config.swapchain_len(), config.v_sync()) };

        Self {
            config,
            event_loop: Some(event_loop),
            gpu,
            #[cfg(debug_assertions)]
            started: Instant::now(),
            swapchain,
            window,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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
        builder = builder.with_fullscreen(Some(Fullscreen::Exclusive(best_video_mode)));

        Self::new_builder(program, config, event_loop, builder)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_window(program: &Program, config: Config) -> Self {
        let dims = config.window_dimensions();
        let mut builder = WindowBuilder::new();
        let event_loop = EventLoop::new();

        // Setup windowed mode
        builder = builder.with_fullscreen(None);

        if let Some(dims) = dims {
            let physical_dims: LogicalSize<_> = dims.into();
            builder = builder.with_inner_size(physical_dims);
        }

        builder = builder.with_min_inner_size(LogicalSize::new(
            MINIMUM_WINDOW_SIZE as f32,
            MINIMUM_WINDOW_SIZE as f32,
        ));

        Self::new_builder(program, config, event_loop, builder)
    }

    /// Borrows the `Gpu` instance.
    pub fn gpu(&self) -> &Gpu<P> {
        &self.gpu
    }

    #[cfg(debug_assertions)]
    fn perf_begin(&mut self) {
        info!("Starting event loop");

        self.started = Instant::now();
    }

    #[cfg(debug_assertions)]
    fn perf_tick(&mut self, render_buf_len: usize) {
        let now = Instant::now();
        let elapsed = now - self.started;
        self.started = now;

        let fps = (1_000_000_000.0 / elapsed.as_nanos() as f64) as usize;
        match fps {
            fps if fps >= 59 => debug!(
                "Frame complete: {}ns ({}fps buf={})",
                elapsed.as_nanos().to_formatted_string(&Locale::en),
                fps.to_formatted_string(&Locale::en),
                render_buf_len,
            ),
            fps if fps >= 50 => info!(
                "Frame complete: {}ns ({}fps buf={}) (FRAME DROPPED)",
                elapsed.as_nanos().to_formatted_string(&Locale::en),
                fps.to_formatted_string(&Locale::en),
                render_buf_len,
            ),
            _ => warn!(
                "Frame complete: {}ns ({}fps buf={}) (STALLED)",
                elapsed.as_nanos().to_formatted_string(&Locale::en),
                fps.to_formatted_string(&Locale::en),
                render_buf_len,
            ),
        }
    }

    unsafe fn present(&mut self, mut frame: Render<P>, buf: &mut VecDeque<Box<dyn Op<P>>>) {
        let mut ops = frame.drain_ops().peekable();

        // We work-around this condition, below, but it is not expected that a well-formed
        // program would never do this. It causes undefined behavior when passing a frame with no
        // operations.
        debug_assert!(ops.peek().is_some());

        // Pop completed operations off the back of the buffer ...
        while let Some(op) = buf.back() {
            if op.is_complete() {
                buf.pop_back().unwrap();
            } else {
                break;
            }
        }

        // ... and push new operations onto the front.
        let had_ops = ops.peek().is_some();
        for op in ops {
            buf.push_front(op);
        }

        if had_ops {
            // Target can be dropped directly after presentation, it will return to the pool. If for
            // some reason the pool is drained before the hardware is finished with target the
            // underlying texture is still referenced by the operations.
            self.swapchain.present(frame.as_ref());
        }
    }

    /// Runs a program starting with the given `DynScreen`.
    ///
    /// Immediately after this call, `render` will be called on the screen, followed by `update`, ad
    /// infinium. This call does not return to the calling code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # fn __() {
    /// fn main() {
    ///     let engine = Engine::default();
    ///     engine.run(Box::new(FooScreen)) // <- Note the return value which is the no-return bang
    ///                                     //    "value", inception. 🤯
    /// }
    ///
    /// struct FooScreen;
    ///
    /// impl Screen<RcK> for FooScreen {
    ///     // not shown
    /// # #[cfg(not(feature = "multi-monitor"))] fn render(&self, _: &Gpu, _: Extent) -> Render
    /// # { todo!(); }
    /// # #[cfg(feature = "multi-monitor")] fn render(&self, _: &Gpu, _: &[Area]) ->
    /// # Vec<Option<Render>> { todo!(); }
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
    /// }
    /// # }
    /// ```
    pub fn run(mut self, screen: DynScreen<P>) -> ! {
        #[cfg(debug_assertions)]
        self.perf_begin();

        let mut input = Input::default();
        let mut render_buf = VecDeque::with_capacity(DEFAULT_RENDER_BUF_LEN);
        let mut screen: Option<DynScreen<P>> = Some(screen);
        let event_loop = self.event_loop.take().unwrap();

        // Pump events until the application exits
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent { event, window_id } => match event {
                    WindowEvent::CloseRequested if window_id == self.window.id() => {
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::KeyboardInput {
                        input: keyboard_input,
                        ..
                    } => input.key.handle(&keyboard_input),
                    WindowEvent::Resized(dims) => {
                        let dims: Extent = dims.into();

                        info!("Window resized to {}x{}", dims.x, dims.y);

                        self.swapchain.set_dims(dims);
                    }
                    _ => {}
                },
                Event::MainEventsCleared => self.window.request_redraw(),
                Event::RedrawRequested(_) => {
                    // Render & present the screen, saving the ops in our buffer
                    unsafe {
                        self.present(
                            screen
                                .as_ref()
                                .unwrap()
                                .render(&self.gpu, self.swapchain.dims()),
                            &mut render_buf,
                        );
                    }

                    // Update the current scene state, potentially returning a new one
                    screen = Some(screen.take().unwrap().update(&self.gpu, &input));

                    // We have handled all input
                    input.key.clear();

                    #[cfg(debug_assertions)]
                    self.perf_tick(render_buf.len());
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
        Self::new(
            #[cfg(not(target_arch = "wasm32"))]
            Program::default(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<P> From<Program<'_, '_>> for Engine<P>
where
    P: SharedPointerKind,
{
    fn from(program: Program<'_, '_>) -> Self {
        Self::new(program)
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
/// ```rust
/// # use screen_13::prelude_rc::*;
/// # use std::fs::File;
/// # type PakFile = Pak<File>;
/// # struct FooScreen { bar: Shared<Bitmap>, }
/// impl FooScreen {
///     fn load(gpu: &Gpu, pak: &mut PakFile) -> Self {
///         Self {
///             bar: gpu.read_bitmap(pak, "bar"),
///         }
///     }
/// }
/// ```
///
/// Example load during `render` (_`update` works too_):
///
/// ```rust
/// # use screen_13::prelude_rc::*;
/// # use std::cell::RefCell;
/// # use std::fs::File;
/// # use std::io::BufReader;
/// # struct FooScreen { bar: RefCell<Option<Shared<Bitmap>>>, pak: RefCell<Pak<BufReader<File>>>, }
/// impl Screen<RcK> for FooScreen {
///     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
///         *self.bar.borrow_mut() = Some(gpu.read_bitmap(&mut self.pak.borrow_mut(), "bar"));
///         
///         todo!("🎨🖼️ render something awesome!");
///     }
///
///     // update not shown
/// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
/// }
/// ```
pub trait Screen<P>
where
    P: 'static + SharedPointerKind,
{
    /// When paired with an `Engine`, generates images presented to the physical display adapter
    /// using a swapchain and fullscreen video mode or operating system window.
    ///
    /// # Examples
    ///
    /// Calling `render` on another `Screen`:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # fn __() {
    /// # let gpu = Gpu::offscreen();
    /// # let foo = Solid::new(GREEN);
    /// // "foo" is a DynScreen, let's ask it to render a document!
    /// let mut foo_doc = foo.render(&gpu, Extent::new(1024, 128));
    ///
    /// // 🤮 Ugh! I didn't like it!
    /// foo_doc.clear().record();
    ///
    /// println!("{:?}", foo_doc);
    /// # }
    /// ```
    ///
    /// Responding to `render` as a `Screen` implementation:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # struct Foo;
    /// # impl Screen<RcK> for Foo {
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
    /// fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///     let mut frame = gpu.render(dims);
    ///
    ///     // 🥇 It's some of my best work!
    ///     frame.clear().with(GREEN).record();
    ///
    ///     frame
    /// }
    /// # }
    /// ```
    ///
    /// **_NOTE:_** It is considered undefined behavior to return a render which has not recorded
    /// any commands, as shown:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # struct Foo;
    /// # impl Screen<RcK> for Foo {
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
    /// fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///     // This is UB because the graphics hardware might have been using this render to store
    ///     // an 8K atlas of 😸's, and it is not guaranteed to be initialized.
    ///     // Hey, the more you know!
    ///     gpu.render(dims)
    /// }
    /// # }
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
    #[cfg(not(feature = "multi-monitor"))]
    fn render(&self, gpu: &Gpu<P>, dims: Extent) -> Render<P>;

    /// When paired with an `Engine`, generates images presented to the physical display adapter
    /// using a swapchain and fullscreen video mode or operating system window.
    ///
    /// # Examples
    ///
    /// Calling `render` on another `Screen`:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # fn __() {
    /// # let gpu = Gpu::offscreen();
    /// # let foo = Solid::new(GREEN);
    /// // "foo" is a DynScreen, let's ask it to render some documents!
    /// let foo_docs = foo.render(&gpu, &[Extent::new(1024, 128)]);
    ///
    /// // 🤮 Ugh! I didn't like them!
    /// foo_docs.for_each(|doc| doc.clear().record());
    ///
    /// println!("{:?}", foo_docs);
    /// # }
    /// ```
    ///
    /// Responding to `render` as a `Screen` implementation:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # struct Foo;
    /// # impl Screen<Rc> for Foo {
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
    /// fn render(&self, gpu: &Gpu, viewports: &[Area]) -> Vec<Option<Render<P>>> {
    ///     viewports.iter().map(|viewport| {
    ///         let frame = gpu.render(dims);
    ///         
    ///         // 🥇 It's some of my best work!
    ///         frame.clear().with(GREEN).record();
    ///
    ///         Some(frame)
    ///     }).collect()
    /// }
    /// # }
    /// ```
    ///
    /// **_NOTE:_** It is considered undefined behavior to return a render which has not recorded
    /// any commands, as shown:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # struct Foo;
    /// # impl Screen<Rc> for Foo {
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { }
    /// fn render(&self, gpu: &Gpu, viewports: &[Area]) -> Vec<Option<Render<P>>> {
    ///     // This is UB because the graphics hardware might have been using this render to store
    ///     // an 8K atlas of 😸's, and it is not guaranteed to be initialized.
    ///     // Hey, the more you know!
    ///     viewports.iter().map(|viewport| gpu.render(viewport.dims)).collect()
    /// }
    /// # }
    /// ```
    #[cfg(feature = "multi-monitor")]
    fn render(&self, gpu: &Gpu<P>, viewports: &[Area]) -> Vec<Option<Render<P>>>;

    /// Responds to user input and either provides a new `DynScreen` instance or `self` to indicate
    /// no-change.
    ///
    /// After `update`, `render` will be called on the returned screen, and the previous screen will
    /// be dropped.
    ///
    /// # Examples
    ///
    /// Render this screen forever, never responding to user input or exiting:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # struct Foo;
    /// # impl Screen<RcK> for Foo {
    /// # #[cfg(not(feature = "multi-monitor"))] fn render(&self, _: &Gpu, _: Extent) -> Render
    /// # { todo!(); }
    /// # #[cfg(feature = "multi-monitor")] fn render(&self, _: &Gpu, _: &[Area]) ->
    /// # Vec<Option<Render>> { todo!(); }
    /// fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
    ///     // 🙈 Yolo!
    ///     self
    /// }
    /// # }
    /// ```
    ///
    /// A kind of three way junction. Goes to `BarScreen` when Home is pressed, otherwise
    /// presents the current screen, rendering for five seconds before quitting:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # use std::process::exit;
    /// # struct BarScreen;
    /// # impl Screen<RcK> for BarScreen {
    /// # #[cfg(not(feature = "multi-monitor"))] fn render(&self, _: &Gpu, _: Extent) -> Render
    /// # { todo!(); }
    /// # #[cfg(feature = "multi-monitor")] fn render(&self, _: &Gpu, _: &[Area]) ->
    /// # Vec<Option<Render>> { todo!(); }
    /// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!() }
    /// # }
    /// # struct Foo { wall_time: f32, }
    /// # impl Screen<RcK> for Foo {
    /// # #[cfg(not(feature = "multi-monitor"))] fn render(&self, _: &Gpu, _: Extent) -> Render
    /// # { todo!(); }
    /// # #[cfg(feature = "multi-monitor")] fn render(&self, _: &Gpu, _: &[Area]) ->
    /// # Vec<Option<Render>> { todo!(); }
    /// fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
    ///     if input.keys.is_key_down(Key::Home) {
    ///         Box::new(BarScreen)
    ///     } else if self.wall_time < 5.0 {
    ///         self
    ///     } else {
    ///         // 👋
    ///         exit(0);
    ///     }
    /// }
    /// # }
    /// ```
    fn update(self: Box<Self>, gpu: &Gpu<P>, input: &Input) -> DynScreen<P>;
}
