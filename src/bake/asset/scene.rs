use {
    super::Model,
    crate::math::{vec3, EulerRot, Quat, Vec3},
    ordered_float::OrderedFloat,
    serde::{
        de::{self, value::{MapAccessDeserializer, SeqAccessDeserializer}, MapAccess, SeqAccess, Visitor},
        Deserialize, Deserializer,
    },
    std::{
        fmt,
        f32::consts::PI,
        path::{Path, PathBuf},
    },
};


#[derive(Clone, Eq, Hash, PartialEq)]
struct ModelRef {
    asset: Option<Model>,
    src: Option<PathBuf>,
}

impl ModelRef {
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
    fn de<'de, D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ModelRefVisitor;

        impl<'de> Visitor<'de> for ModelRefVisitor {
            type Value = Option<ModelRef>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("path string or model asset")
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(ModelRef {
                    asset: Some(asset),
                    src: None,
                }))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Some(ModelRef {
                    asset: None,
                    src: Some(PathBuf::from(str)),
                }))
            }
        }

        deserializer.deserialize_any(ModelRefVisitor)
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

/// Holds a description of one scene reference.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct SceneRef {
    id: Option<String>,

    #[serde(default, deserialize_with = "ModelRef::de")]
    model: Option<ModelRef>,

    material: Option<PathBuf>,
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
    pub fn model(&self) -> Option<&Path> {
        self.model.as_ref().map(|model| model.src.as_deref()).flatten()
    }

    /// Optional direct reference to a material asset file.
    ///
    /// If specified, the material asset does not need to be referenced in any content file. If the
    /// material is referenced in a content file it will not be duplicated or cause any problems.
    pub fn material(&self) -> Option<&Path> {
        self.material.as_deref()
    }

    /// Any 3D position or position-like data.
    pub fn position(&self) -> Vec3 {
        self.position
            .map(|position| vec3(position[0].0, position[1].0, position[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Any 3D orientation  or orientation-like data.
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
