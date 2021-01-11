//! Contains color types and functions.
//!
//! ## Note
//!
//! I'm not sure this module has enough value on its own; other crates provide good API for this. Hmm.

mod alpha;
mod qbasic;

pub use self::{
    alpha::{AlphaColor, TRANSPARENT_BLACK},
    qbasic::color,
};

use {
    crate::math::{vec3, Vec3},
    gfx_hal::command::{ClearColor, ClearValue},
};

/// Black - 0x000000
pub const BLACK: Color = Color::rgb(0, 0, 0);

/// Blue - 0x0000ff
pub const BLUE: Color = Color::rgb(0, 0, 255);

/// Cornflower Blue - 0x6495ed
pub const CORNFLOWER_BLUE: Color = Color::rgb(100, 149, 237);

/// Magenta - 0xff00ff
pub const MAGENTA: Color = Color::rgb(255, 0, 255);

/// Green - 0x00ff00
pub const GREEN: Color = Color::rgb(0, 255, 0);

/// Red - 0xff0000
pub const RED: Color = Color::rgb(255, 0, 0);

/// White - 0xffffff
pub const WHITE: Color = Color::rgb(255, 255, 255);

/// A three channel color
#[derive(Clone, Copy, Debug, Default)]
pub struct Color {
    /// Blue channel
    pub b: u8,

    /// Green channel
    pub g: u8,

    /// Red channel
    pub r: u8,
}

impl Color {
    /// Constructs a `Color` from the given values.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { b, g, r }
    }

    /// Returns a hexadecimal color code as a string.
    pub fn to_hex(self) -> String {
        format!("#{:x}{:x}{:x}", self.r, self.g, self.b)
    }

    /// Constructs a `Vec3` from this color.
    pub fn to_rgb(self) -> Vec3 {
        const SCALE: f32 = 1.0 / u8::MAX as f32;

        vec3(
            self.r as f32 * SCALE,
            self.g as f32 * SCALE,
            self.b as f32 * SCALE,
        )
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self { b, g, r }
    }
}

impl From<AlphaColor> for Color {
    fn from(alpha_color: AlphaColor) -> Self {
        Self {
            b: alpha_color.b,
            g: alpha_color.g,
            r: alpha_color.r,
        }
    }
}

impl From<ClearValue> for Color {
    fn from(clear_value: ClearValue) -> Self {
        unsafe {
            Self {
                b: (clear_value.color.float32[0] * 255.0) as _,
                g: (clear_value.color.float32[1] * 255.0) as _,
                r: (clear_value.color.float32[2] * 255.0) as _,
            }
        }
    }
}

impl From<Color> for ClearValue {
    fn from(color: Color) -> Self {
        let alpha_color: AlphaColor = color.into();
        let color = alpha_color.to_rgba();
        Self {
            color: ClearColor {
                float32: [color.x, color.y, color.z, color.w],
            },
        }
    }
}
