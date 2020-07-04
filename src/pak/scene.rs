use {
    crate::math::{vec3, Quat, Vec3},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneRef {
    pub id: String,
    pub key: String,
    pub position: (f32, f32, f32),
    pub roll_pitch_yaw: (f32, f32, f32),
    pub tags: Vec<String>,
}

impl SceneRef {
    pub fn is_tagged(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_owned())
    }

    pub fn position(&self) -> Vec3 {
        vec3(self.position.0, self.position.1, self.position.2)
    }

    pub fn rotation(&self) -> Quat {
        //let rpw = self.roll_pitch_yaw;
        //Quat::from_euler_angles(rpw.0, rpw.1, rpw.2)
        todo!("Result should be normalized")
    }
}
