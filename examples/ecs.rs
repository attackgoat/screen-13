// NOTE: This is a somewhat advanced example, and it uses a few external crates!

#[macro_use]
extern crate log;

#[macro_use]
extern crate paste;

/// This macro creates a HECS-compatible 'system' which adapts the HECS API to Screen 13
macro_rules! ecs_sys {
    ($name: ident) => {
        paste! {
            #[allow(dead_code)]
            mod [<$name:snake _sys>] {
                use {
                    super::PakFile,
                    screen_13::{gpu::{Gpu, [<$name Ref>]}, pak::[<$name Id>]},
                    std::collections::HashMap,
                };

                #[derive(Clone, Copy)]
                pub struct Ref {
                    id: [<$name Id>],
                }

                #[derive(Default)]
                pub struct System {
                    [<$name:snake s>]: HashMap<[<$name Id>], [<$name Ref>]>,
                }

                impl System {
                    pub fn [<$name:snake>](&self, [<$name:snake>]: Ref) -> [<$name Ref>] {
                        [<$name Ref>]::clone(&self.[<$name:snake s>][&[<$name:snake>].id])
                    }

                    pub fn load<K: AsRef<str>>(&mut self, gpu: &Gpu, pak: &mut PakFile, key: K) -> Ref {
                        let key = key.as_ref();
                        let id = pak.[<$name:snake _id>](key).unwrap();
                        self.load_with_id(gpu, pak, id)
                    }

                    pub fn load_ref<K: AsRef<str>>(
                        &mut self,
                        gpu: &Gpu,
                        pak: &mut PakFile,
                        key: K,
                    ) -> [<$name Ref>] {
                        let [<$name:snake>] = self.load(gpu, pak, key);
                        self.[<$name:snake>]([<$name:snake>])
                    }

                    pub fn load_with_id(&mut self, gpu: &Gpu, pak: &mut PakFile, id: [<$name Id>]) -> Ref {
                        if self.[<$name:snake s>].contains_key(&id) {
                            return Ref { id };
                        }

                        debug!("Loading [<$name:snake>] {:?}", id);

                        self.[<$name:snake s>].insert(
                            id,
                            [<$name Ref>]::new(gpu.[<read_ $name:snake _with_id>](
                                pak,
                                id,
                            )),
                        );

                        Ref { id }
                    }
                }
            }
        }
    };
}

ecs_sys!(Bitmap);
ecs_sys!(Model);

use {
    self::{
        bitmap_sys::{Ref as Bitmap, System as BitmapSystem},
        model_sys::{Ref as Model, System as ModelSystem},
    },
    hecs::World,
    screen_13::prelude_all::*,
    std::{cell::RefCell, env::current_exe, fs::File, io::BufReader},
};

type PakFile = Pak<BufReader<File>>;

fn main() -> ! {
    pretty_env_logger::init();

    // Get the engine ready ...
    let engine = Engine::default();
    let pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ecs.pak"),
    )
    .expect("ERROR: You must first pack the runtime content into a file by running the following command: `cargo run examples/content/ecs.toml`");
    let mut game = Game::new(pak);

    // ... load and play the game
    game.load(engine.gpu());
    engine.run(Box::new(game))
}

struct Game {
    bitmaps: BitmapSystem,
    draws: RefCell<Vec<Draw>>,
    ecs: World,
    models: ModelSystem,
    pak: PakFile,
}

impl Game {
    fn new(pak: PakFile) -> Self {
        Self {
            bitmaps: Default::default(),
            draws: Default::default(),
            ecs: Default::default(),
            models: Default::default(),
            pak,
        }
    }

    fn load(&mut self, gpu: &Gpu) {
        // HINT: This could use a Scene asset file to automate this
        let textured_material = self.load_material(gpu, "material/box_textured.toml");
        let box_model = self
            .models
            .load(gpu, &mut self.pak, "gltf/box_textured.toml");

        // Make a 3x3 grid of boxes, spawned using the HECS system
        for y in -1..1 {
            for x in -1..1 {
                let position: Position = vec3(x as f32 * 1.5, y as f32 * 1.5, 0.5).into();
                let rotation: Rotation = Quat::identity().into();
                self.ecs
                    .spawn((box_model, textured_material, position, rotation));
            }
        }

        // Add a light via HECS
        let position: Position = Vec3::zero().into();
        let light = Light {
            color: WHITE,
            power: 60.0,
            radius: 1.0,
        };
        self.ecs.spawn((light, position));
    }

    fn load_material(&mut self, gpu: &Gpu, key: &'static str) -> Material {
        let material = self.pak.material(key);

        Material {
            color: self
                .bitmaps
                .load_with_id(gpu, &mut self.pak, material.color),
            metal_rough: self
                .bitmaps
                .load_with_id(gpu, &mut self.pak, material.metal_rough),
            normal: self
                .bitmaps
                .load_with_id(gpu, &mut self.pak, material.normal),
        }
    }
}

impl Screen for Game {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        // Use HECS to query and fill a local draw cache
        let mut draws = self.draws.borrow_mut();
        draws.clear();
        draws.extend(
            self.ecs
                .query::<(&Model, &Material, &Position, &Rotation)>()
                .iter()
                .map(|(_, (model, material, position, rotation))| {
                    let model = self.models.model(*model);
                    let material = screen_13::gpu::Material {
                        color: self.bitmaps.bitmap(material.color),
                        metal_rough: self.bitmaps.bitmap(material.metal_rough),
                        normal: self.bitmaps.bitmap(material.normal),
                    };
                    let transform =
                        Mat4::from_rotation_translation(rotation.into(), position.into());
                    Draw::model(model, material, transform)
                }),
        );
        draws.extend(self.ecs.query::<(&Light, &Position)>().iter().map(
            |(_, (light, position))| {
                Draw::point_light(position.into(), light.color, light.power, light.radius)
            },
        ));

        let camera = Perspective::default();

        // Renders the ECS-generated draws on a black background
        let mut frame = gpu.render(dims);
        frame.clear().record();
        frame.draw().record(&camera, &mut draws);
        frame
    }

    fn update(self: Box<Self>, _: &Gpu, _: &Input) -> DynScreen {
        self
    }
}

#[derive(Clone, Copy)]
pub struct Light {
    pub color: Color,
    pub power: f32,
    pub radius: f32,
}

#[derive(Clone, Copy)]
pub struct Material {
    pub color: Bitmap,
    pub metal_rough: Bitmap,
    pub normal: Bitmap,
}

#[derive(Clone, Copy)]
pub struct Position(pub Vec3);

impl From<Vec3> for Position {
    fn from(val: Vec3) -> Self {
        Self(val)
    }
}

impl From<Position> for Vec3 {
    fn from(val: Position) -> Self {
        val.0
    }
}

impl From<&Position> for Vec3 {
    fn from(val: &Position) -> Self {
        val.0
    }
}

#[derive(Clone, Copy)]
pub struct Rotation(pub Quat);

impl From<Quat> for Rotation {
    fn from(val: Quat) -> Self {
        Self(val)
    }
}

impl From<Rotation> for Quat {
    fn from(val: Rotation) -> Self {
        val.0
    }
}

impl From<&Rotation> for Quat {
    fn from(val: &Rotation) -> Self {
        val.0
    }
}
