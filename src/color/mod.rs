mod alpha;
mod qbasic;

pub use self::{
    alpha::{AlphaColor, TRANSPARENT_BLACK},
    qbasic::color,
};

use {
    crate::math::{vec3, Vec3},
    gfx_hal::{
        command::{ClearColor, ClearValue},
        format::Format,
    },
};

pub const BLACK: Color = Color::rgb(0, 0, 0);
pub const BLUE: Color = Color::rgb(0, 0, 255);
pub const CORNFLOWER_BLUE: Color = Color::rgb(154, 206, 235);
pub const MAGENTA: Color = Color::rgb(255, 0, 255);
pub const GREEN: Color = Color::rgb(0, 255, 0);
pub const RED: Color = Color::rgb(255, 0, 0);
pub const WHITE: Color = Color::rgb(255, 255, 255);

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub b: u8,
    pub g: u8,
    pub r: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { b, g, r }
    }

    pub fn swizzle(self, _format: Format) -> Self {
        // match format.base_format().0 {
        //     SurfaceType::B8_G8_R8_A8 => Self::rgb(self.b, self.g, self.r),
        //     _ => self,
        // }
        self
    }

    pub fn to_hex(self) -> String {
        format!("#{:x}{:x}{:x}", self.r, self.g, self.b)
    }

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
        let (r, g, b, a) = alpha_color.to_unorm();
        Self {
            color: ClearColor {
                float32: [r, g, b, a],
            },
        }
    }
}
