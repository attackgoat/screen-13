use {screen_13::prelude_rc::*, std::env::current_exe};

/// This example requires a color graphics adapter.
fn main() -> ! {
    pretty_env_logger::init();

    let engine = Engine::new(Program::default().with_window());

    // Open the "pak" file which contains all game art, assests, and other content
    let mut pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("basic.pak"),
    )
    .expect("ERROR: You must first pack the runtime content into a file by running the following command: `cargo run examples/res/basic.toml`");

    // Initialize our "game" by loading everything it requires to run
    let small_10px = engine.gpu().read_font(&mut pak, "font/small_10px");

    // Voila!
    engine.run(Box::new(Basic { small_10px }))
}

struct Basic {
    small_10px: Font,
}

impl Screen<RcK> for Basic {
    fn render(&self, gpu: &Gpu, _: Extent) -> Render {
        // This creates a canvas-like "Render" type which we can use to record graphic commands
        // We will use it to render a fixed-size retro-resolution screen
        let mut frame = gpu.render(Extent::new(320, 200));

        // Draws "Hello, World" onto a blue background
        frame.clear().with(BLUE).record();
        frame
            .text(Coord::new(137, 96), WHITE)
            .record(&self.small_10px, "Hello, world!");

        // Present the completed frame to the screen
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        // This screen never transitions to any other screen; and it does not respond to any input
        self
    }
}
