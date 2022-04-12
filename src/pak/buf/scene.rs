use {
    super::{
        file_key, is_toml, material::Material, model::Model, parent, Asset, Canonicalize,
        SceneBuf, SceneId,
    },
    crate::pak::SceneRefData,
    glam::{vec3, EulerRot, Quat, Vec3},
    log::info,
    ordered_float::OrderedFloat,
    serde::{
        de::{value::MapAccessDeserializer, MapAccess, Visitor},
        Deserialize, Deserializer,
    },
    std::{
        f32::consts::PI,
        fmt::Formatter,
        io::Error,
        marker::PhantomData,
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use {super::Writer, parking_lot::Mutex, std::sync::Arc, tokio::runtime::Runtime};

/// A reference to a model asset or model source file.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AssetRef<T> {
    /// A `Model` asset specified inline.
    Asset(T),

    /// A `Model` asset file or model source file.
    Path(PathBuf),
}

impl<'de, T> AssetRef<T>
where
    T: Deserialize<'de>,
{
    /// Deserialize from any of absent or:
    ///
    /// src of file.gltf:
    /// .. = "file.gltf"
    ///
    /// src of file.toml which must be a Model asset:
    /// .. = "file.toml"
    ///
    /// src of a Model asset:
    /// .. = { src = "file.gltf" }
    fn de<D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct AssetRefVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for AssetRefVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Option<AssetRef<T>>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("path string or model asset")
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(AssetRef::Asset(asset)))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(AssetRef::Path(PathBuf::from(str))))
            }
        }

        deserializer.deserialize_any(AssetRefVisitor(PhantomData))
    }
}

impl<T> Canonicalize for AssetRef<T>
where
    T: Canonicalize,
{
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        match self {
            Self::Asset(asset) => asset.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
        }
    }
}

/// Holds a description of position/orientation/scale and tagged data specific to each program.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Scene {
    #[serde(rename = "ref")]
    refs: Vec<SceneRef>,
}

impl Scene {
    /// Reads and processes scene source files into an existing `.pak` file buffer.
    #[cfg(feature = "bake")]
    pub fn bake(
        &self,
        rt: &Runtime,
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
        src: impl AsRef<Path>,
    ) -> Result<SceneId, Error> {
        // Early-out if we have already baked this scene
        let asset = self.clone().into();
        if let Some(h) = writer.lock().ctx.get(&asset) {
            return Ok(h.as_scene().unwrap());
        }

        let key = file_key(&project_dir, &src);

        info!("Baking scene: {}", key);

        let src_dir = parent(&src);

        let mut refs = vec![];
        for scene_ref in self.refs() {
            // all tags must be lower case (no localized text!)
            let mut tags = vec![];
            for tag in scene_ref.tags() {
                let baked = tag.as_str().trim().to_lowercase();
                if let Err(idx) = tags.binary_search(&baked) {
                    tags.insert(idx, baked);
                }
            }

            let material = scene_ref
                .material()
                .map(|material| match material {
                    AssetRef::Asset(material) => {
                        // Material asset specified inline
                        let mut material = material.clone();
                        material.canonicalize(&project_dir, &src_dir);
                        (None, material)
                    }
                    AssetRef::Path(src) => {
                        let src = Material::canonicalize_project_path(&project_dir, &src_dir, src);
                        if is_toml(&src) {
                            // Asset file reference
                            let mut material = Asset::read(&src).unwrap().into_material().unwrap(); // TODO: UNWRAP!
                            material.canonicalize(&project_dir, &src_dir);
                            (Some(src), material)
                        } else {
                            // Material color file reference
                            (None, Material::new(src))
                        }
                    }
                })
                .map(|(src, mut material)| {
                    material
                        .bake(rt, writer, &project_dir, &src_dir, src)
                        .expect("material")
                });

            let model = scene_ref
                .model()
                .map(|model| match model {
                    AssetRef::Asset(model) => {
                        // Model asset specified inline
                        let mut model = model.clone();
                        model.canonicalize(&project_dir, &src_dir);
                        (None, model)
                    }
                    AssetRef::Path(src) => {
                        let src = Model::canonicalize_project_path(&project_dir, &src_dir, src);
                        if is_toml(&src) {
                            // Asset file reference
                            let mut model = Asset::read(&src).unwrap().into_model().expect("model");
                            model.canonicalize(&project_dir, &src_dir);
                            (Some(src), model)
                        } else {
                            // Model file reference
                            (None, Model::new(src))
                        }
                    }
                })
                .map(|(src, model)| model.bake(writer, &project_dir, src).expect("bake model"));

            refs.push(SceneRefData {
                id: scene_ref.id().map(|id| id.to_owned()),
                material,
                model,
                position: scene_ref.position(),
                rotation: scene_ref.rotation(),
                tags,
            });
        }

        let scene = SceneBuf::new(refs.into_iter());

        let mut writer = writer.lock();
        if let Some(h) = writer.ctx.get(&asset) {
            return Ok(h.as_scene().unwrap());
        }

        Ok(writer.push_scene(scene, key))
    }

    /// Individual references within a scene.
    #[allow(unused)]
    pub fn refs(&self) -> &[SceneRef] {
        &self.refs
    }
}

impl Canonicalize for Scene {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.refs
            .iter_mut()
            .for_each(|scene_ref| scene_ref.canonicalize(&project_dir, &src_dir));
    }
}

/// Holds a description of one scene reference.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct SceneRef {
    id: Option<String>,

    #[serde(default, deserialize_with = "AssetRef::<Material>::de")]
    material: Option<AssetRef<Material>>,

    #[serde(default, deserialize_with = "AssetRef::<Model>::de")]
    model: Option<AssetRef<Model>>,

    position: Option<[OrderedFloat<f32>; 3]>,
    rotation: Option<[OrderedFloat<f32>; 3]>,
    tags: Option<Vec<String>>,
}

impl SceneRef {
    /// Main identifier of a reference, not required to be unique.
    #[allow(unused)]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Optional direct reference to a model asset file.
    ///
    /// If specified, the model asset does not need to be referenced in any content file. If the
    /// model is referenced in a content file it will not be duplicated or cause any problems.
    ///
    /// May either be a `Model` asset specified inline or a model source file. Model source files
    /// may be either `.toml` `Model` asset files or direct references to `.glb`/`.gltf` files.
    #[allow(unused)]
    pub fn model(&self) -> Option<&AssetRef<Model>> {
        self.model.as_ref()
    }

    /// Optional direct reference to a material asset file.
    ///
    /// If specified, the material asset does not need to be referenced in any content file. If the
    /// material is referenced in a content file it will not be duplicated or cause any problems.
    #[allow(unused)]
    pub fn material(&self) -> Option<&AssetRef<Material>> {
        self.material.as_ref()
    }

    /// Any 3D position or position-like data.
    #[allow(unused)]
    pub fn position(&self) -> Vec3 {
        self.position
            .map(|position| vec3(position[0].0, position[1].0, position[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Any 3D orientation or orientation-like data.
    #[allow(unused)]
    pub fn rotation(&self) -> Quat {
        let rotation = self
            .rotation
            .map(|rotation| vec3(rotation[0].0, rotation[1].0, rotation[2].0))
            .unwrap_or(Vec3::ZERO)
            * PI
            / 180.0;

        // x = pitch
        // y = yaw
        // z = roll
        Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z)
    }

    /// An arbitrary collection of program-specific strings.
    #[allow(unused)]
    pub fn tags(&self) -> &[String] {
        self.tags.as_deref().unwrap_or(&[])
    }
}

impl Canonicalize for SceneRef {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        if let Some(material) = self.material.as_mut() {
            material.canonicalize(&project_dir, &src_dir);
        }

        if let Some(model) = self.model.as_mut() {
            model.canonicalize(&project_dir, &src_dir);
        }
    }
}
