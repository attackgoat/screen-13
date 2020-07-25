use {
    super::{vec3_is_finite, Vec3},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Plane {
    distance: f32,
    normal: Vec3,
}

impl Plane {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        assert!(distance.is_finite());
        assert!(vec3_is_finite(normal));
        assert!(normal.is_normalized());

        Self { distance, normal }
    }

    /// Returns the distance in the direction of `normal` from the origin to this plane.
    pub const fn distance(&self) -> f32 {
        self.distance
    }

    /// Returns the orientation of this plane.
    pub const fn normal(&self) -> Vec3 {
        self.normal
    }
}
