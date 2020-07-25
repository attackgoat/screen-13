use {
    super::{vec3_is_finite, Vec3},
    serde::{Deserialize, Serialize},
    std::ops::{Add, AddAssign},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Sphere {
    center: Vec3,
    radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        assert!(vec3_is_finite(center));
        assert!(radius.is_finite());
        assert!(radius > 0.0);

        Self { center, radius }
    }

    /// Returns the average of all points of this sphere.
    pub const fn center(&self) -> Vec3 {
        self.center
    }

    /// Returns the maximum distance between any two points of this sphere.
    pub fn diameter(&self) -> f32 {
        self.radius * 2.0
    }

    /// Returns the distance from `center` to any point on the surface of this sphere.
    pub const fn radius(&self) -> f32 {
        self.radius
    }
}

impl<T> Add<T> for Sphere
where
    T: Into<f32>,
{
    type Output = Self;

    fn add(self, val: T) -> Self {
        Self {
            center: self.center,
            radius: self.radius + val.into(),
        }
    }
}

impl<T> AddAssign<T> for Sphere
where
    T: Into<f32>,
{
    fn add_assign(&mut self, val: T) {
        *self = *self + val.into();
    }
}
