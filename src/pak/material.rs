use {
    super::BitmapId,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Material {
    pub(crate) color: BitmapId,
    pub(crate) metal_rough: BitmapId,
    pub(crate) normal: BitmapId,
}

impl Material {
    pub fn color(&self) -> BitmapId {
        self.color
    }

    pub fn metal_rough(&self) -> BitmapId {
        self.metal_rough
    }

    pub fn normal(&self) -> BitmapId {
        self.normal
    }
}
