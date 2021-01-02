use {
    super::{vec3, vec3_is_finite, Vec3},
    serde::{Deserialize, Serialize},
    std::ops::{Add, AddAssign},
};

/// A geometrical object in three-dimensional space that is the surface of a ball.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Sphere {
    center: Vec3,
    radius: f32,
}

impl Sphere {
    /// Constructs a sphere from the given center and radius.
    pub fn new(center: Vec3, radius: f32) -> Self {
        assert!(vec3_is_finite(center));
        assert!(radius.is_finite());
        assert!(radius > 0.0);

        Self { center, radius }
    }

    /// Constructs a sphere from the given list of positions.
    pub fn from_point_cloud<I: Iterator<Item = Vec3>>(cloud: I) -> Self {
        let cloud = cloud.collect::<Vec<_>>();

        let mut center = Vec3::zero();
        for point in &cloud {
            center += *point;
        }

        let count = cloud.len() as f32;
        center /= vec3(count, count, count);

        let mut distance_squared = 0.0f32;
        for point in &cloud {
            distance_squared = distance_squared.max(center.distance_squared(*point));
        }

        Self {
            center,
            radius: distance_squared.sqrt(),
        }
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

impl<I> From<I> for Sphere
where
    I: Iterator<Item = Vec3>,
{
    fn from(cloud: I) -> Self {
        Self::from_point_cloud(cloud)
    }
}
