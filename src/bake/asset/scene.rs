use {
    crate::math::{Quat, Vec3},
    serde::{Deserialize, Serialize},
    std::f32::consts::PI,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Scene {
    refs: Vec<Ref>,
}

impl Scene {
    pub fn refs(&self) -> &[Ref] {
        &self.refs
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Ref {
    id: Option<String>,
    key: Option<String>,
    position: Option<Vec3>,
    rotation: Option<Vec3>,
    tags: Option<Vec<String>>,
}

impl Ref {
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    pub fn position(&self) -> Vec3 {
        self.position.unwrap_or(Vec3::zero())
    }

    pub fn rotation(&self) -> Quat {
        let rotation = self.rotation.unwrap_or(Vec3::zero());
        let x = rotation.x() * 180.0 * PI;
        let y = rotation.y() * 180.0 * PI;
        let z = rotation.z() * 180.0 * PI;
        Quat::from_rotation_ypr(y, x, z)
    }

    pub fn tags(&self) -> &[String] {
        self.tags.as_deref().unwrap_or(&[])
    }
}
