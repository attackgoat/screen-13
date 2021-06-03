use {
    crate::math::{EulerRot, Quat, Vec3},
    serde::Deserialize,
    std::{
        f32::consts::PI,
        path::{Path, PathBuf},
    },
};

/// Holds a description of position/orientation/scale and tagged data specific to each program.
#[derive(Clone, Deserialize)]
pub struct Scene {
    #[serde(rename = "ref")]
    refs: Vec<Ref>,
}

impl Scene {
    /// Individual references within a scene.
    pub fn refs(&self) -> &[Ref] {
        &self.refs
    }
}

/// Holds a description of one scene reference.
#[derive(Clone, Deserialize)]
pub struct Ref {
    id: Option<String>,
    model: Option<PathBuf>,
    material: Option<PathBuf>,
    position: Option<Vec3>,
    rotation: Option<Vec3>,
    tags: Option<Vec<String>>,
}

impl Ref {
    /// Main identifier of a reference, not required to be unique.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Optional direct reference to a model asset file.
    ///
    /// If specified, the model asset does not need to be referenced in any content file. If the
    /// model is referenced in a content file it will not be duplicated or cause any problems.
    pub fn model(&self) -> Option<&Path> {
        self.model.as_deref()
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
        self.position.unwrap_or(Vec3::ZERO)
    }

    /// Any 3D orientation  or orientation-like data.
    pub fn rotation(&self) -> Quat {
        let rotation = self.rotation.unwrap_or(Vec3::ZERO) * PI / 180.0;

        // y = yaw
        // x = pitch
        // z = roll
        Quat::from_euler(EulerRot::YXZ, rotation.y, rotation.x, rotation.z)
    }

    /// An arbitrary collection of program-specific strings.
    pub fn tags(&self) -> &[String] {
        self.tags.as_deref().unwrap_or(&[])
    }
}
