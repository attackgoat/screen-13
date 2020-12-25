use {
    crate::{
        gpu::{BlendMode, Write, WriteMode},
        math::{Coord, Extent, Rect},
        DynScreen, Gpu, Input, Render, Screen,
    },
    std::{
        time::{Duration, Instant},
        u8,
    },
};

// TODO: Specialize with FadeIn, FadeOut, CrossFade versions
/// Fades between two screens.
///
/// Remark: Screens are only drawn, and not updated, during fade.
pub struct Fade {
    a: Option<DynScreen>,
    b: Option<DynScreen>,
    duration: Duration,
    mode: BlendMode,
    started: Instant,
}

impl Fade {
    pub fn new(a: DynScreen, b: DynScreen, duration: Duration) -> Self {
        Self {
            a: Some(a),
            b: Some(b),
            duration,
            mode: Default::default(),
            started: Instant::now(),
        }
    }

    pub fn with_blend_mode(&mut self, mode: BlendMode) {
        self.mode = mode;
    }
}

impl Screen for Fade {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
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
            #[cfg(debug_assertions)]
            "Fade write B",
            WriteMode::Blend((ab, self.mode)),
        )
        .record(&mut [Write::region(
            &b,
            Rect {
                pos: Coord::ZERO,
                dims,
            },
        )]);

        a
    }

    fn update(mut self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        if Instant::now() - self.started > self.duration {
            self.b.take().unwrap()
        } else {
            self
        }
    }
}
