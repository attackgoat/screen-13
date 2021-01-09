use {
    crate::{
        gpu::{
            write::{Write, WriteMode},
            BlendMode,
        },
        math::{Coord, Extent, Rect},
        ptr::Shared,
        DynScreen, Gpu, Input, Render, Screen,
    },
    archery::SharedPointerKind,
    std::{
        time::{Duration, Instant},
        u8,
    },
};

// TODO: Specialize with FadeIn, FadeOut, CrossFade versions
/// Visually fades between two `Screen` implementations over time.
///
/// ## Examples
///
/// In order to fade from `Foo` to `Bar` you might:
///
/// ```
/// use {screen_13::prelude_all::*, std::time::Duration};
///
/// fn main() {
///     Engine::default().run(Box::new(Foo))
/// }
///
/// struct Foo;
///
/// impl Screen for Foo {
///     ...
///
///     fn update(self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
///         let b = Box::new(bar);
///         let t = Duration::from_secs(1.0);
///
///         // The Fade type will call render on (Foo) and bar for u, how handy! 🤖
///         Fade::new(self, b, t)
///     }
/// }
///
/// struct Bar;
///
/// impl Screen for Bar {
///     ...
/// }
///
/// ```
///
/// _Note:_ Screens are only drawn, and not updated, during fade.
pub struct Fade<P>
where
    P: SharedPointerKind,
{
    a: Option<DynScreen<P>>,
    b: Option<DynScreen<P>>,
    duration: Duration,
    mode: BlendMode,
    started: Instant,
}

impl<P> Fade<P>
where
    P: SharedPointerKind,
{
    /// Constructs a `Fade` from the given `a` and `b` screens and duration.
    pub fn new(a: DynScreen<P>, b: DynScreen<P>, duration: Duration) -> Self {
        Self {
            a: Some(a),
            b: Some(b),
            duration,
            mode: Default::default(),
            started: Instant::now(),
        }
    }

    /// Sets the blend mode for this fade.
    pub fn with_blend_mode(&mut self, mode: BlendMode) {
        self.mode = mode;
    }
}

impl<P> Screen<P> for Fade<P>
where
    P: 'static + SharedPointerKind,
{
    fn render(&self, gpu: &Gpu<P>, dims: Extent) -> Render<P> {
        // Render each of the a and b screens normally
        let mut a = self.a.as_ref().unwrap().render(gpu, dims);
        let b = self.b.as_ref().unwrap().render(gpu, dims);

        // Figure out `ab` which is 0..1 as we fade from a to b
        let elapsed = match Instant::now() - self.started {
            elapsed if elapsed < self.duration => elapsed,
            _ => self.duration,
        };
        let ab = ((elapsed.as_millis() as f32 / self.duration.as_millis() as f32).min(1.0)
            * u8::MAX as f32) as u8;

        #[cfg(debug_assertions)]
        debug!("Fade AB: {}", ab);

        let dims = b.dims();
        let b = gpu.resolve(b);

        a.write(
            #[cfg(feature = "debug-names")]
            "Fade write B",
        )
        .with_mode(WriteMode::Blend((ab, self.mode)))
        .record(&mut [Write::region(
            &b,
            Rect {
                pos: Coord::ZERO,
                dims,
            },
        )]);

        a
    }

    fn update(mut self: Box<Self>, _: &Gpu<P>, _: &Input) -> DynScreen<P> {
        if Instant::now() - self.started > self.duration {
            self.b.take().unwrap()
        } else {
            self
        }
    }
}
