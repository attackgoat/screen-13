mod line;
mod rect_light;
mod spotlight;

pub use self::{line::LineKey, rect_light::RectLightKey, spotlight::SpotlightKey};

use std::u8;

const BIT: f32 = 1.0 / u8::MAX as f32;

#[derive(Clone, Copy)]
pub enum Key {
    Line(LineKey),
    RectLight(RectLightKey),
    Spotlight(SpotlightKey),
}

impl From<LineKey> for Key {
    fn from(val: LineKey) -> Self {
        Self::Line(val)
    }
}

impl From<RectLightKey> for Key {
    fn from(val: RectLightKey) -> Self {
        Self::RectLight(val)
    }
}

impl From<SpotlightKey> for Key {
    fn from(val: SpotlightKey) -> Self {
        Self::Spotlight(val)
    }
}
