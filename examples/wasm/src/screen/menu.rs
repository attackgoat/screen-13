use screen_13::prelude_rc::*;

pub struct Menu {
    pub font_h1: Shared<BitmapFont>,
}

impl Screen<RcK> for Menu {
    fn render(&self, gpu: &Gpu, _: Extent) -> Render {
        // This creates a canvas-like "Render" type which we can use to record graphic commands
        // We will use it to render a fixed-size retro-resolution screen
        let mut frame = gpu.render(Extent::new(320, 200));

        // Draws "Hello, World" onto a blue background
        frame.clear().with(BLUE).record();
        frame.text().record(&mut [Text::position(
            Coord::new(137, 96),
            &self.font_h1,
            "Hello, world!",
        )]);

        // Present the completed frame to the screen
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        self
    }
}
