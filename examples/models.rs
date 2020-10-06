use {
    screen_13::{
        camera::Perspective,
        color::WHITE,
        gpu::{Command, Font, Model},
        math::Coord,
        pak::Pak,
        prelude::*,
    },
    std::env::current_exe,
};

fn main() -> ! {
    // Note: There are instructions in the content/khronos_group/README.md which you will need to follow in order to obtain the models
    let engine = Engine::new(Program::new("screen-13-models-example"));
    let mut pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("models.pak"),
    )
    .expect(
        "ERROR: You must first run the following command: `cargo run examples/content/models.s13`",
    );

    let font = engine.gpu().load_font(&mut pak, "small_10px");
    let models = vec![(
        engine.gpu().load_model(
            #[cfg(debug_assertions)]
            "flight_helmet",
            &mut pak,
            "khronos_group/flight_helmet",
        ),
        String::from("Flight Helmet"),
    )];

    engine.run(Box::new(Display {
        font,
        model_idx: 0,
        models,
    }));
}

struct Display {
    font: Font,
    model_idx: usize,
    models: Vec<(Model, String)>,
}

impl Screen for Display {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            "models render",
            dims,
        );

        frame.draw(
            #[cfg(debug_assertions)]
            "basic line",
            &Perspective::new(
                screen_13::math::vec3(0.0, 0.0, 0.0),
                screen_13::math::vec3(0.0, 0.0, 1.0),
                0.5..1.5,
                45.0,
                0.5,
            ),
            &mut [Command::line(
                screen_13::math::vec3(0.0, 0.0, 0.0),
                screen_13::color::RED,
                screen_13::math::vec3(320.0, 200.0, 0.0),
                screen_13::color::BLUE,
            )],
        );
        frame.text(
            #[cfg(debug_assertions)]
            "model name",
            &self.font,
            &self.models[self.model_idx].1,
            Coord::new(2, 10),
            WHITE,
        );

        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        // This screen never transitions to any other screen; and it does not respond to any input
        self
    }
}
