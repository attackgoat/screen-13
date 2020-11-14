use {
    crate::math::{Quat, Vec3},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneRef {
    id: Option<String>,
    key: Option<String>,
    position: Vec3,
    rotation: Quat,
    tags: Vec<String>,
}

impl SceneRef {
    pub fn new(
        id: Option<String>,
        key: Option<String>,
        position: Vec3,
        rotation: Quat,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id,
            key,
            position,
            rotation,
            tags,
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_owned())
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    // pub fn tags(&self) -> &[&str] {
    //     self.tags.as_slice()
    // }
}
