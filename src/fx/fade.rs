use {
    crate::{
        gpu::{BlendMode, WriteMode},
        math::Coord,
        DynScreen, Gpu, Input, Render, Screen,
    },
    std::{
        time::{Duration, Instant},
        u8,
    },
};

// TODO: Specialize with FadeIn, FadeOut, CrossFade versions
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
    fn render(&self, gpu: &Gpu) -> Render {
        let origin = Coord::zero();
        let mut a = self.a.as_ref().unwrap().render(gpu);
        let mut b = self.b.as_ref().unwrap().render(gpu);

        // Figure out `ab` which is 0..255 as we fade from a to b
        let elapsed = match Instant::now() - self.started {
            elapsed if elapsed < self.duration => elapsed,
            _ => self.duration,
        };
        let ab = ((elapsed.as_millis() as f32 / self.duration.as_millis() as f32).min(1.0)
            * u8::MAX as f32) as u8;

        #[cfg(debug_assertions)]
        debug!("Fade AB: {}", ab);

        let dims = b.dims();
        let (b, b_ops) = b.resolve();
        a.extend_ops(b_ops);

        a.write_mode(
            #[cfg(debug_assertions)]
            "Fade write B",
            b.as_ref(),
            origin,
            dims,
            WriteMode::Blend((ab, self.mode)),
        );

        a
    }

    fn update(mut self: Box<Self>, gpu: &Gpu, input: &Input) -> DynScreen {
        if Instant::now() - self.started > self.duration {
            self.b.take().unwrap()
        } else {
            self.a = Some(self.a.take().unwrap().update(gpu, input));
            self.b = Some(self.b.take().unwrap().update(gpu, input));
            self
        }
    }
}
