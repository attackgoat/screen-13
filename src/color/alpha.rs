use {
    super::Color,
    crate::math::{vec4, Vec4},
    gfx_hal::{
        command::{ClearColor, ClearValue},
        image::PackedColor,
    },
    std::u8,
};

pub const TRANSPARENT_BLACK: AlphaColor = AlphaColor::rgba(0, 0, 0, 0);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaColor {
    pub a: u8,
    pub b: u8,
    pub g: u8,
    pub r: u8,
}

impl AlphaColor {
    pub fn is_transparent(self) -> bool {
        self.a < u8::MAX
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { b, g, r, a: 0xff }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { b, g, r, a }
    }

    pub fn to_rgba(self) -> Vec4 {
        const SCALE: f32 = 1.0 / u8::MAX as f32;

        vec4(
            self.r as f32 * SCALE,
            self.g as f32 * SCALE,
            self.b as f32 * SCALE,
            self.a as f32 * SCALE,
        )
    }
}

impl Default for AlphaColor {
    fn default() -> Self {
        TRANSPARENT_BLACK
    }
}

impl From<Color> for AlphaColor {
    fn from(color: Color) -> Self {
        Self {
            a: 0xff,
            b: color.b,
            g: color.g,
            r: color.r,
        }
    }
}

impl From<AlphaColor> for ClearValue {
    fn from(color: AlphaColor) -> Self {
        let color = color.to_rgba();
        Self {
            color: ClearColor {
                float32: [color.x, color.y, color.z, color.w],
            },
        }
    }
}

impl From<AlphaColor> for PackedColor {
    fn from(color: AlphaColor) -> Self {
        Self(
            (color.r as u32) << 24
                | (color.g as u32) << 16
                | (color.b as u32) << 8
                | (color.a as u32),
        )
    }
}
