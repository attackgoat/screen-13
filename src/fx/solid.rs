use crate::{color::Color, math::Extent, DynScreen, Gpu, Input, Render, Screen};

pub struct Solid {
    color: Color,
}

impl Solid {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Screen for Solid {
    fn render(&self, gpu: &Gpu) -> Render {
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            &format!("Solid {}", self.color.to_hex()),
            Extent::new(8, 8),
        );
        frame.clear(self.color);

        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        self
    }
}
