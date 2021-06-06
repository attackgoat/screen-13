//! Mathematics types and functions, mostly based on
//! [_glam-rs_](https://github.com/bitshifter/glam-rs).
//!
//! Also contains some geometric math types.

mod cone;
mod coord;
mod plane;
mod rect;
mod sphere;

pub use {
    self::{cone::Cone, plane::Plane, sphere::Sphere},
    glam::{mat4, quat, vec2, vec3, vec4, EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4},
};

use self::{coord::Coord as GenericCoord, rect::Rect as GenericRect};

/// A rectangular area with u32 position and size values.
pub type Area = GenericRect<u32, u32>;

/// A coordinate with u8 values.
pub type Coord8 = GenericCoord<u8>;

/// A coordinate with i32 values.
pub type Coord = GenericCoord<i32>;

/// A coordinate with f32 values.
pub type CoordF = GenericCoord<f32>;

/// A coordinate with u32 values.
pub type Extent = GenericCoord<u32>;

/// A rectangular area with i32 position and u32 size values.
pub type Rect = GenericRect<u32, i32>;

/// A rectangular area with f32 position and size values.
pub type RectF = GenericRect<f32, f32>;

/// Returns `true` if the given vector is neither infinite nor `NaN`.
#[inline]
pub fn vec2_is_finite(val: Vec2) -> bool {
    let x = val.x.is_finite() as u8;
    let y = val.y.is_finite() as u8;

    x * y == 1
}

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

/// Returns `true` if the given vector is neither infinite nor `NaN`.
#[inline]
pub fn vec4_is_finite(val: Vec4) -> bool {
    // Use saturating casts so we can swap three `jbe` instructions for two `and` instructions.
    // Probably not needed but branchless code sure is fun: https://godbolt.org/z/P58cdq
    let x = val.x.is_finite() as u8;
    let y = val.y.is_finite() as u8;
    let z = val.z.is_finite() as u8;
    let w = val.w.is_finite() as u8;

    x * y * z * w == 1
}

/// Creates a Vec4 from a Vec3 and f32 value.
#[inline]
pub fn vec4_from_vec3(vec: Vec3, w: f32) -> Vec4 {
    vec4(vec.x, vec.y, vec.z, w)
}
