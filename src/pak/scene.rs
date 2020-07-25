use {
    crate::math::{Quat, Vec3},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneRef {
    id: String,
    key: String,
    pos: Vec3,
    rot: Vec3,
    tags: Vec<String>,
}

impl SceneRef {
    pub fn new(id: String, key: String, pos: Vec3, rot: Vec3, tags: Vec<String>) -> Self {
        Self {
            id,
            key,
            pos,
            rot,
            tags,
        }
    }

    pub fn is_tagged(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_owned())
    }

    pub fn position(&self) -> Vec3 {
        self.pos
    }

    pub fn rotation(&self) -> Quat {
        //let rpw = self.roll_pitch_yaw;
        //Quat::from_euler_angles(rpw.0, rpw.1, rpw.2)
        todo!("Result should be normalized")
    }
}
