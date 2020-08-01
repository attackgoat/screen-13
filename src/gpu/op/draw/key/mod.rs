mod line;
mod rect_light;
mod spotlight;

pub use self::{line::LineKey, rect_light::RectLightKey, spotlight::SpotlightKey};

use std::u8;

const BIT: f32 = 1.0 / u8::MAX as f32;
