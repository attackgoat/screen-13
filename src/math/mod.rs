mod coord;
mod rect;

pub use glam::{mat4, quat, vec2, vec3, vec4, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

use self::{coord::Coord as GenericCoord, rect::Rect as GenericRect};

pub type Coord = GenericCoord<i32>;
pub type CoordF = GenericCoord<f32>;
pub type Extent = GenericCoord<u32>;
pub type Rect = GenericRect<i32, u32>;
pub type RectF = GenericRect<f32, f32>;

pub fn vec4_from_vec3(vec: Vec3, w: f32) -> Vec4 {
    vec4(vec.x(), vec.y(), vec.z(), w)
}
