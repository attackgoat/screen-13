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

    let rye_regular = engine
        .gpu()
        .read_scalable_font(&mut pak, "font/rye_regular");

    engine.run(Box::new(Example { rye_regular }))
}

struct Example {
    rye_regular: Shared<ScalableFont>,
}

impl Screen<RcK> for Example {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let mut frame = gpu.render(dims);
        frame.clear().record();
        frame.text().record(&mut [Text::position(
            Coord::new(137, 96),
            &self.rye_regular,
            "Hello, world!",
        )]);
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        if input.key.any_down() {
            exit(0);
        }

        self
    }
}
