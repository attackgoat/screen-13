use {
    super::BitmapId,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Material {
    pub(crate) albedo: BitmapId,
    pub(crate) metal_rough: BitmapId,
    pub(crate) normal: BitmapId,
}

impl Material {
    pub fn albedo(&self) -> BitmapId {
        self.albedo
    }

    pub fn metal_rough(&self) -> BitmapId {
        self.metal_rough
    }

    pub fn normal(&self) -> BitmapId {
        self.normal
    }
}
