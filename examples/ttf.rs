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
    let rye_regular = engine
        .gpu()
        .read_vector_font(&mut pak, "font/rye_regular.ttf", 32.0);

    engine.run(Box::new(Example {
        cedarville_cursive,
        rye_regular,
    }))
}

struct Example {
    cedarville_cursive: Shared<VectorFont>,
    rye_regular: Shared<VectorFont>,
}

impl Screen<RcK> for Example {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let mut frame = gpu.render(dims);
        frame.clear().with(WHITE).record();
        frame.text().record(&mut [
            VectorText::position(Coord::new(5, 50), &self.rye_regular, "Rye Regular")
                .with_glygh_color(RED)
                .build(),
            VectorText::position(
                Coord::new(5, 150),
                &self.cedarville_cursive,
                "Cedarville Cursive",
            )
            .build(),
        ]);
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        if input.key.any_down() {
            exit(0);
        }

        self
    }
}
