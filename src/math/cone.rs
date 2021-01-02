use {
    super::{vec3_is_finite, Vec3},
    serde::{Deserialize, Serialize},
};

/// A three-dimensional geometric shape that tapers smoothly from a circular base to a point called the apex.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Cone {
    apex: Vec3,
    height: f32, // TODO: Store with normal as direction?
    normal: Vec3,
    radius: f32,
}

impl Cone {
    /// Constructs a new cone from values.
    pub fn new(apex: Vec3, normal: Vec3, height: f32, radius: f32) -> Self {
        assert!(vec3_is_finite(apex));
        assert!(vec3_is_finite(normal));
        assert!(normal.is_normalized());
        assert!(height > 0.0);
        assert!(radius > 0.0);

        Self {
            apex,
            height,
            normal,
            radius,
        }
    }

    /// Returns the position of the vertex at the pointy end of this cone.
    pub const fn apex(&self) -> Vec3 {
        self.apex
    }

    /// Returns the distance from the base to the `apex` of this cone.
    pub const fn height(&self) -> f32 {
        self.height
    }

    /// Returns the direction from `apex` towards the base of this cone.
    pub const fn normal(&self) -> Vec3 {
        self.normal
    }

    /// Returns the radius of the base of this cone.
    pub const fn radius(&self) -> f32 {
        self.radius
    }
}
