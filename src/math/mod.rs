mod cone;
mod coord;
mod plane;
mod rect;
mod sphere;

pub use {
    self::{cone::Cone, plane::Plane, sphere::Sphere},
    glam::{mat4, quat, vec2, vec3, vec4, Mat3, Mat4, Quat, Vec2, Vec3, Vec4},
};

use self::{coord::Coord as GenericCoord, rect::Rect as GenericRect};

pub type Area = GenericRect<u32, u32>;
pub type Coord8 = GenericCoord<u8>;
pub type Coord = GenericCoord<i32>;
pub type CoordF = GenericCoord<f32>;
pub type Extent = GenericCoord<u32>;
pub type Rect = GenericRect<u32, i32>;
pub type RectF = GenericRect<f32, f32>;

/// Returns `true` if the given vector is neither infinite nor `NaN`.
#[inline]
pub fn vec3_is_finite(val: Vec3) -> bool {
    // Use saturating casts so we can swap three `jbe` instructions for two `and` instructions.
    // Probably not needed but branchless code sure is fun: https://godbolt.org/z/P58cdq
    let x = val.x.is_finite() as u8;
    let y = val.y.is_finite() as u8;
    let z = val.z.is_finite() as u8;

    x * y * z == 1
}

#[inline]
pub fn vec4_from_vec3(vec: Vec3, w: f32) -> Vec4 {
    vec4(vec.x, vec.y, vec.z, w)
}
