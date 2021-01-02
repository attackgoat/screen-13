use {
    super::{vec3_is_finite, Vec3},
    serde::{Deserialize, Serialize},
};

/// A flat two-dimensional surface that extends infinitely far.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Plane {
    distance: f32,
    normal: Vec3,
}

impl Plane {
    /// Constructs a new plane from the given normal and distance from the origin.
    pub fn new(normal: Vec3, distance: f32) -> Self {
        debug_assert!(distance.is_finite());
        debug_assert!(vec3_is_finite(normal));
        debug_assert!(normal.is_normalized());

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
