//! This example demonstrates loading scenes as top-level items and also nested within other
//! scenes.
//!
//! If you inspect the `car_city.toml` content file you will notice it references a few cars
//! directly and also the `city.toml` scene file. The cars themselves are also scene files in order
//! to make layout of the wheels and selection of the wheel models easier. The city scene file
//! references all the static scenery, lights, and road data.
//!
//! At load time a HashMap is used to cache the shared model references so that duplicate wheels and
//! scenery models are not loaded twice. All models stored in a single `.pak` file will be uniquely
//! stored once for each given content file key. See the output of the content build process for
//! more information. All models are referred to using an ID which is used for caching.

use {
    lazy_static::lazy_static,
    screen_13::prelude_rc::*,
    std::{
        collections::HashMap,
        env::current_exe,
        io::{Read, Seek},
        path::PathBuf,
    },
};

const PAK_ERROR: &'static str = "ERROR: You must first pack the runtime content into a file by \
    running the following command: `cargo run examples/res/car_city.toml`";

lazy_static! {
    static ref PAK_PATH: PathBuf = current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("car_city.pak");
}

fn main() -> ! {
    pretty_env_logger::init();

    let engine = Engine::new(Program::default().with_window());
    let gpu = engine.gpu();
    let mut pak = Pak::open(PAK_PATH.as_path()).expect(PAK_ERROR);
    let city = City::read(gpu, &mut pak);

    engine.run(Box::new(city))
}

/// Helper for reading a shared scene item model from the cache
fn read_scene_ref_model<R>(
    gpu: &Gpu,
    pak: &mut Pak<R>,
    cache: &mut HashMap<ModelId, Shared<Model>>,
    scene: &Scene,
    key: &'static str,
) -> Shared<Model>
where
    R: Read + Seek,
{
    scene
        .refs()
        .find(|item| item.id() == Some(key))
        .map(|item| {
            let model_id = item.model().unwrap();
            Shared::clone(
                cache
                    .entry(model_id)
                    .or_insert_with(|| gpu.read_model_with_id(pak, model_id)),
            )
        })
        .unwrap()
}

/// Helper for reading a scene item position
fn read_scene_ref_position(scene: &Scene, key: &'static str) -> Vec3 {
    scene
        .refs()
        .find(|item| item.id() == Some(key))
        .map(|item| item.position())
        .unwrap()
}

struct Car {
    axle_front: f32,
    axle_back: f32,
    axle_half_width: f32,
    axle_y: f32,
    body: Shared<Model>,
    wheel: Shared<Model>,
    steering_angle: f32,
}

impl Car {
    fn read<R: Read + Seek>(
        gpu: &Gpu,
        pak: &mut Pak<R>,
        cache: &mut HashMap<ModelId, Shared<Model>>,
        key: &'static str,
    ) -> Self {
        let scene = pak.read_scene(key);
        let body = read_scene_ref_model(gpu, pak, cache, &scene, "body");
        let wheel = read_scene_ref_model(gpu, pak, cache, &scene, "wheel");
        let front_left = read_scene_ref_position(&scene, "front_left");
        let back_left = read_scene_ref_position(&scene, "back_left");

        Self {
            axle_front: front_left.z,
            axle_back: back_left.z,
            axle_half_width: front_left.x,
            axle_y: front_left.y,
            body,
            wheel,
            steering_angle: 0.0,
        }
    }
}

struct City {
    ambulance: Car,
    fire_truck: Car,
    police_car: Car,
    race_car: Car,
    scenery: Vec<Scenery>,
}

impl City {
    fn read<R>(gpu: &Gpu, pak: &mut Pak<R>) -> Self
    where
        R: Read + Seek,
    {
        let mut cache = HashMap::new();
        let scene = pak.read_scene("scene/city");
        let scenery = scene
            .refs()
            .filter(|item| item.model().is_some())
            .map(|item| Scenery::read(gpu, pak, &mut cache, item))
            .collect();

        Self {
            ambulance: Car::read(gpu, pak, &mut cache, "gltf/car_kit/ambulance"),
            fire_truck: Car::read(gpu, pak, &mut cache, "gltf/car_kit/firetruck"),
            police_car: Car::read(gpu, pak, &mut cache, "gltf/car_kit/police"),
            race_car: Car::read(gpu, pak, &mut cache, "gltf/car_kit/race"),
            scenery,
        }

        // Note: The cache is dropped because shared references exist in the cars and scenery items
    }
}

impl Screen<RcK> for City {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let camera = Perspective::new(
            vec3(0.0, 0.0, 10.0),
            vec3(0.0, 0.0, 0.0),
            0.1..20.0,
            45.0,
            dims.x as f32 / dims.y as f32,
        );

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

struct Scenery {
    model: Shared<Model>,
    position: Vec3,
    rotation: Quat,
}

impl Scenery {
    fn read<R>(
        gpu: &Gpu,
        pak: &mut Pak<R>,
        cache: &mut HashMap<ModelId, Shared<Model>>,
        item: Ref,
    ) -> Self
    where
        R: Read + Seek,
    {
        let model_id = item.model().unwrap();
        let model = Shared::clone(
            cache
                .entry(model_id)
                .or_insert_with(|| gpu.read_model_with_id(pak, model_id)),
        );

        Self {
            model,
            position: item.position(),
            rotation: item.rotation(),
        }
    }
}
