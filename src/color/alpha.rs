// TODO: Probably get rid of the hal types here

use {
    super::Color,
    crate::math::{vec4, Vec4},
    gfx_hal::{
        command::{ClearColor, ClearValue},
        image::PackedColor,
    },
    std::u8,
};

/// Black with zero alpha - 0x00000000
pub const TRANSPARENT_BLACK: AlphaColor = AlphaColor::rgba(0, 0, 0, 0);

/// A four channel (with alpha) color.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaColor {
    /// Alpha channel.
    pub a: u8,

    /// Blue channel.
    pub b: u8,

    /// Green channel.
    pub g: u8,

    /// Red channel.
    pub r: u8,
}

impl AlphaColor {
    /// Returns true if the alpha channel is non-zero
    pub fn is_transparent(self) -> bool {
        self.a < u8::MAX
    }

    /// Constructs an `AlphaColor` from the given values.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { a: 0xff, b, g, r }
    }

    /// Constructs an `AlphaColor` from the given values.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { a, b, g, r }
    }

    /// Constructs a `Vec4` from this color.
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

#[cfg(test)]
mod test {
    use super::*;

    const EPSILON: f32 = 0.01;

    #[test]
    fn ctor_rgb() {
        let val = AlphaColor::rgb(1, 2, 3);

        assert_eq!(val.r, 1);
        assert_eq!(val.g, 2);
        assert_eq!(val.b, 3);
        assert_eq!(val.a, 0xff);
    }

    #[test]
    fn ctor_rgba() {
        let val = AlphaColor::rgba(1, 2, 3, 4);

        assert_eq!(val.r, 1);
        assert_eq!(val.g, 2);
        assert_eq!(val.b, 3);
        assert_eq!(val.a, 4);
    }

    #[test]
    fn from_color() {
        let val = AlphaColor::from(Color::rgb(7, 8, 9));

        assert_eq!(val.r, 7);
        assert_eq!(val.g, 8);
        assert_eq!(val.b, 9);
        assert_eq!(val.a, 0xff);
    }

    #[test]
    fn is_transparent() {
        // Transparent colors
        assert!(AlphaColor::rgba(0, 0, 0, 0x00).is_transparent());
        assert!(AlphaColor::rgba(0, 0, 0, 0x01).is_transparent());
        assert!(AlphaColor::rgba(0, 0, 0, 0xfe).is_transparent());

        // Opaque color
        assert!(!AlphaColor::rgba(0, 0, 0, 0xff).is_transparent());
    }

    #[test]
    fn to_clear() {
        let val: ClearValue = AlphaColor::rgba(1, 2, 3, 4).into();

        unsafe {
            assert!((val.color.float32[0] - 1.0 / 255.0) < EPSILON);
            assert!((val.color.float32[1] - 2.0 / 255.0) < EPSILON);
            assert!((val.color.float32[2] - 3.0 / 255.0) < EPSILON);
            assert!((val.color.float32[3] - 4.0 / 255.0) < EPSILON);
        }
    }

    #[test]
    fn to_packed() {
        let val: PackedColor = AlphaColor::rgba(1, 2, 3, 4).into();

        assert_eq!(val.0 >> 24 & 0xff, 1);
        assert_eq!(val.0 >> 16 & 0xff, 2);
        assert_eq!(val.0 >> 8 & 0xff, 3);
        assert_eq!(val.0 & 0xff, 4);
    }

    #[test]
    fn to_rgba() {
        let val = AlphaColor::rgb(0, 0, 0).to_rgba();
        assert_eq!(val.x, 0.0);
        assert_eq!(AlphaColor::rgb(0, 0, 0).to_rgba().y, 0.0);
        assert_eq!(AlphaColor::rgb(0, 0, 0).to_rgba().z, 0.0);
        assert_eq!(AlphaColor::rgb(0, 0, 0).to_rgba().w, 1.0);

        let val = AlphaColor::rgba(0xff, 0x7f, 0xff, 0x00).to_rgba();
        assert_eq!(val.x, 1.0);
        assert!((val.y - 0.5).abs() < EPSILON);
        assert_eq!(val.z, 1.0);
        assert_eq!(val.w, 0.0);
    }
}
