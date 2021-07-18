use {
    screen_13::prelude_rc::*,
    std::{env::current_exe, process::exit},
};

fn main() -> ! {
    pretty_env_logger::init();

    let engine = Engine::new(Program::default().with_window());
    let mut pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ttf.pak"),
    )
    .expect("ERROR: You must first pack the runtime content into a file by running the following command: `cargo run examples/res/ttf.toml`");

    let cedarville_cursive =
        engine
            .gpu()
            .read_vector_font(&mut pak, "font/cedarville_cursive_regular.ttf", 32.0);
    let permanent_marker = engine.gpu().load_vector_font(
        include_bytes!("wasm/res/font/PermanentMarker-Regular.ttf") as &[u8],
        64.0,
    );
    let rye_regular = engine
        .gpu()
        .read_vector_font(&mut pak, "font/rye_regular.ttf", 32.0);

    engine.run(Box::new(Example {
        cedarville_cursive,
        frame: 0,
        permanent_marker,
        rye_regular,
    }))
}

struct Example {
    cedarville_cursive: Shared<VectorFont>,
    frame: usize,
    permanent_marker: Shared<VectorFont>,
    rye_regular: Shared<VectorFont>,
}

impl Screen<RcK> for Example {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let mut frame = gpu.render(dims);
        frame.clear().with(WHITE).record();
        frame.text().record(&mut [
            VectorText::position(Coord::new(0, 128), &self.rye_regular, "Rye Regular")
                .with_size(128.0)
                .with_glygh_color(RED)
                .build(),
            VectorText::position(
                Coord::new(0, 250),
                &self.cedarville_cursive,
                "Cedarville Cursive",
            )
            .with_size(100.0)
            .with_glygh_color(GREEN)
            .build(),
            VectorText::position(
                Coord::new(0, 350),
                &self.permanent_marker,
                "Permanent Marker",
            )
            .with_size(85.0)
            .with_glygh_color(BLUE)
            .build(),
        ]);

        // Just for fun draw a frame counter with a variable size - this puts the dynamic glyph
        // atlas through a few test cases
        frame.text().record(&mut [VectorText::position(
            Coord::new(260, 500),
            &self.rye_regular,
            format!("Frame {}", self.frame),
        )
        .with_size((self.frame as f32 % 180.0).to_radians().sin() * 32.0 + 10.0)
        .with_glygh_color(BLACK)
        .build()]);

        frame
    }

    fn update(mut self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        if input.key.any_down() {
            exit(0);
        }

        self.frame += 1;

        self
    }
}
