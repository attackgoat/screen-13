use {
    super::BitmapId,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Material {
    pub(crate) albedo: BitmapId,
    pub(crate) metal: BitmapId,
    pub(crate) normal: BitmapId,
}

impl Material {
    pub fn albedo(&self) -> BitmapId {
        self.albedo
    }

    pub fn metal(&self) -> BitmapId {
        self.metal
    }

    pub fn normal(&self) -> BitmapId {
        self.normal
    }
}
