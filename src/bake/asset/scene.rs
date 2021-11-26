use {
    super::{Canonicalize, Material, Model},
    crate::math::{vec3, EulerRot, Quat, Vec3},
    ordered_float::OrderedFloat,
    serde::{
        de::{self, value::MapAccessDeserializer, MapAccess, Visitor},
        Deserialize, Deserializer,
    },
    std::{
        f32::consts::PI,
        fmt,
        marker::PhantomData,
        path::{Path, PathBuf},
    },
};

/// A reference to a model asset or model source file.
#[derive(Clone, Eq, Hash, PartialEq)]
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

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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
                E: de::Error,
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
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        match self {
            Self::Asset(asset) => asset.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
        }
    }
}

/// Holds a description of position/orientation/scale and tagged data specific to each program.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Scene {
    #[serde(rename = "ref")]
    refs: Vec<SceneRef>,
}

impl Scene {
    /// Individual references within a scene.
    pub fn refs(&self) -> &[SceneRef] {
        &self.refs
    }
}

impl Canonicalize for Scene {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.refs
            .iter_mut()
            .for_each(|scene_ref| scene_ref.canonicalize(&project_dir, &src_dir));
    }
}

/// Holds a description of one scene reference.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
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
    pub fn model(&self) -> Option<&AssetRef<Model>> {
        self.model.as_ref()
    }

    /// Optional direct reference to a material asset file.
    ///
    /// If specified, the material asset does not need to be referenced in any content file. If the
    /// material is referenced in a content file it will not be duplicated or cause any problems.
    pub fn material(&self) -> Option<&AssetRef<Material>> {
        self.material.as_ref()
    }

    /// Any 3D position or position-like data.
    pub fn position(&self) -> Vec3 {
        self.position
            .map(|position| vec3(position[0].0, position[1].0, position[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Any 3D orientation or orientation-like data.
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
    pub fn tags(&self) -> &[String] {
        self.tags.as_deref().unwrap_or(&[])
    }
}

impl Canonicalize for SceneRef {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        if let Some(material) = self.material.as_mut() {
            material.canonicalize(&project_dir, &src_dir);
        }

        if let Some(model) = self.model.as_mut() {
            model.canonicalize(&project_dir, &src_dir);
        }
    }
}
