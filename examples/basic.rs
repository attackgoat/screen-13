use {
    screen_13::{color::qb_color, gpu::Font, math::Coord, pak::Pak, prelude::*},
    std::env::current_exe,
};

// We will render a fixed-size retro-resolution screen
const SCREEN_SIZE: Extent = Extent::new(320, 200);

/// This example requires a color graphics adapter.
fn main() -> ! {
    // Create an engine instance (loads the engine config file for this named game)
    // NOTE: This line also turns on logging so we should do this before anything else
    let engine = Engine::new(Program::new("screen-13-basic-example"));

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
    .expect("ERROR: You must first pack the game content into a file by running the following command: `cargo run examples/content/basic.s13`");

    // Initialize our "game" by loading everything it requires to run
    let small_10px = engine.gpu().load_font(&mut pak, "fonts/small_10px.fnt");

    // Voila!
    engine.run(Box::new(Basic { small_10px }));
}

struct Basic {
    small_10px: Font,
}

impl Screen for Basic {
    fn render(&self, gpu: &Gpu, _: Extent) -> Render {
        // This creates a canvas-like "Render" type which we can use to record graphic commands
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            "basic render",
            SCREEN_SIZE,
        );

        // Draws "Hello, World" onto a blue background
        frame.clear(qb_color(1));
        frame.text(
            #[cfg(debug_assertions)]
            "basic text",
            &self.small_10px,
            "Hello, world!",
            Coord::new(137, 96),
            qb_color(15),
        );
        frame.draw(
            #[cfg(debug_assertions)]
            "basic line",
            &screen_13::camera::Perspective::new(
                screen_13::math::vec3(0.0, 0.0, 0.0),
                screen_13::math::vec3(0.0, 0.0, 1.0),
                0.5..1.5,
                45.0,
                0.5,
            ),
            &mut [screen_13::gpu::Command::line(
                screen_13::math::vec3(0.0, 0.0, 0.0),
                screen_13::color::RED,
                screen_13::math::vec3(320.0, 200.0, 0.0),
                screen_13::color::BLUE,
            )],
        );

        // Present the completed frame to the screen
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        // This screen never transitions to any other screen; and it does not respond to any input
        self
    }
}
