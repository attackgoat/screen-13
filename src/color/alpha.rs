use {
    super::Color,
    gfx_hal::{
        command::{ClearColor, ClearValue},
        image::PackedColor,
    },
    std::u8,
};

pub const TRANSPARENT_BLACK: AlphaColor = AlphaColor::rgba(0, 0, 0, 0);

#[derive(Clone, Copy, Debug)]
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

    pub fn to_rgba_f32(self) -> (f32, f32, f32, f32) {
        (
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        )
    }

    pub fn to_rgba_unorm_u32_array(self) -> [u32; 4] {
        let unorm = self.to_rgba_f32();
        [
            unorm.0.to_bits(),
            unorm.1.to_bits(),
            unorm.2.to_bits(),
            unorm.3.to_bits(),
        ]
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
        let (r, g, b, a) = color.to_rgba_f32();
        Self {
            color: ClearColor {
                float32: [r, g, b, a],
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
