use {
    super::RenderReturn,
    crate::{
        gpu::{
            write::{Write, WriteMode},
            BlendMode, Gpu, Render,
        },
        math::{Coord, Extent, Rect},
        DynScreen, Input, Screen,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    f8::f8,
    std::{
        iter::once,
        time::{Duration, Instant},
    },
};

#[cfg(feature = "multi-monitor")]
use crate::math::Area;

// TODO: Specialize with FadeIn, FadeOut, CrossFade versions
/// Visually fades between two `Screen` implementations over time.
///
/// # Examples
///
/// In order to fade from `Foo` to `Bar` you might:
///
/// ```
/// # use screen_13::prelude_rc::*;
/// # use std::time::Duration;
/// # struct FooScreen;
/// # impl Screen<RcK> for Foo {
/// # fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen { todo!(); }
/// # fn render(&self, _: &Gpu, _: Extent) -> Render { todo!(); }
/// # }
/// # type Bar = Foo;
/// # fn __() {
/// // The DynScreen types do not need specification and are shown for clarity only.
/// let a: DynScreen = Box::new(Foo);
/// let b: DynScreen = Box::new(Bar);
/// let t = Duration::from_secs(1.0);
///
/// // The Fade type will call render on (Foo) and bar for u, how handy! 🤖
/// let c: DynScreen = Fade::new(a, b, t);
/// # }
/// ```
///
/// **_Note:_** Screens are drawn, but not updated, during fade.
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

impl<P> Fade<P>
where
    P: SharedPointerKind,
{
    fn frame(&self, mut a: Render<P>, b: Render<P>, ab: f8) -> Render<P> {
        let dims = b.dims();

        a.write(
            #[cfg(feature = "debug-names")]
            "Fade write B",
        )
        .with_mode(WriteMode::Blend((ab, self.mode)))
        .record(once(Write::region(
            b,
            Rect {
                pos: Coord::ZERO,
                dims,
            },
        )));

        a
    }
}

impl<P> Screen<P> for Fade<P>
where
    P: 'static + SharedPointerKind,
{
    fn render(
        &self,
        gpu: &Gpu<P>,
        #[cfg(not(feature = "multi-monitor"))] dims: Extent,
        #[cfg(feature = "multi-monitor")] viewports: &[Area],
    ) -> RenderReturn<P> {
        // Figure out `ab` which is 0..1 as we fade from a to b
        let elapsed = match Instant::now() - self.started {
            elapsed if elapsed < self.duration => elapsed,
            _ => self.duration,
        };
        let ab = (elapsed.as_millis() as f32 / self.duration.as_millis() as f32).min(1.0);

        // // #[cfg(debug_assertions)]
        // // debug!("Fade AB: {}", ab);

        #[cfg(not(feature = "multi-monitor"))]
        {
            let a = self.a.as_ref().unwrap().render(gpu, dims);
            let b = self.b.as_ref().unwrap().render(gpu, dims);

            self.frame(a, b, ab.into())
        }

        #[cfg(feature = "multi-monitor")]
        {
            self.a
                .as_ref()
                .unwrap()
                .render(gpu, viewports)
                .iter()
                .zip(self.b.as_ref().unwrap().render(gpu, viewports).iter())
                .map(|a, b| self.frame(gpu, ab, a, b))
        }
    }

    fn update(mut self: Box<Self>, _: &Gpu<P>, _: &Input) -> DynScreen<P> {
        if Instant::now() - self.started > self.duration {
            self.b.take().unwrap()
        } else {
            self
        }
    }
}
