use {
    crate::math::{Quat, Vec3},
    serde::Deserialize,
    std::{
        f32::consts::PI,
        path::{Path, PathBuf},
    },
};

#[derive(Clone, Deserialize)]
pub struct Scene {
    #[serde(rename = "ref")]
    refs: Vec<Ref>,
}

impl Scene {
    pub fn refs(&self) -> &[Ref] {
        &self.refs
    }
}

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
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn model(&self) -> Option<&Path> {
        self.model.as_deref()
    }

    pub fn material(&self) -> Option<&Path> {
        self.material.as_deref()
    }

    pub fn position(&self) -> Vec3 {
        self.position.unwrap_or(Vec3::zero())
    }

    pub fn rotation(&self) -> Quat {
        let rotation = self.rotation.unwrap_or(Vec3::zero()) * PI / 180.0;

        Quat::from_rotation_ypr(rotation.y, rotation.x, rotation.z)
    }

    pub fn tags(&self) -> &[String] {
        self.tags.as_deref().unwrap_or(&[])
    }
}
