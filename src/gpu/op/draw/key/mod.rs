mod line;
mod rect_light;
mod spotlight;

pub use self::{line::Line, rect_light::RectLight, spotlight::Spotlight};

use std::u8;

const BIT: f32 = 1.0 / u8::MAX as f32;

pub trait Stride {
    fn stride() -> u64;
}
