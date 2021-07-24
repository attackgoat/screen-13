use {screen_13::prelude_rc::*, std::env::current_exe};

fn main() -> ! {
    pretty_env_logger::init();

    let engine = Engine::new(Program::default().with_window());
    let (character, criminal, idle) = {
    let mut pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("char_anim.pak"),
    )
    .expect("ERROR: You must first pack the runtime content into a file by running the following command: `cargo run examples/res/char_anim.toml`");
    let gpu = engine.gpu();
    let character = gpu
        .read_model(&mut pak, "gltf/character/character");
    let criminal = gpu
        .read_bitmap(&mut pak, "gltf/character/criminal");
    let idle = gpu.read_animation(&mut pak, "gltf/character/idle");

        (character, criminal, idle)
    };

    engine.run(Box::new(CharacterAnimation {
        character,
        criminal,
        idle,
    }))
}

struct CharacterAnimation {
    character: Shared<Model>,
    criminal: Shared<Bitmap>,
    idle: Shared<Animation>,
}

impl Screen<RcK> for CharacterAnimation {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let camera = Perspective::new(vec3(0.0, 0.0, 10.0), vec3(0.0, 0.0, 0.0), 0.1..20.0, 45.0, dims.x as f32 / dims.y as f32);

        let mut frame = gpu.render(dims);
        frame.clear().with(CORNFLOWER_BLUE).record();
        // frame.draw().with_preserve().record(&camera, [
        //     Draw::model(self.character, self.criminal, Mat4::IDENTITY),
        // ]);
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        self
    }
}
