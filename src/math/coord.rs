use {
    super::Vec2,
    gfx_hal::{
        image::{Extent, Offset},
        pso::Rect,
        window::Extent2D,
    },
    serde::{Deserialize, Serialize},
    std::ops::{Div, DivAssign, Mul, MulAssign, Neg},
    winit::dpi::PhysicalSize,
};

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct Coord<T>
where
    T: Sized,
{
    pub x: T,
    pub y: T,
}

impl<T> Coord<T>
where
    T: Sized,
{
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl Coord<f32> {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Returns `true` if this coordinate is neither infinite nor `NaN`.
    pub fn is_finite(self) -> bool {
        let x = self.x.is_finite() as u8;
        let y = self.y.is_finite() as u8;

        x * y == 1
    }
}

impl Coord<i32> {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn as_offset_z(self, z: i32) -> Offset {
        Offset {
            x: self.x,
            y: self.y,
            z,
        }
    }

    pub const fn as_rect_at(self, pos: Self) -> Rect {
        Rect {
            x: pos.x as _,
            y: pos.y as _,
            w: self.x as _,
            h: self.y as _,
        }
    }
}

impl Coord<u32> {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn as_extent_depth(self, depth: u32) -> Extent {
        Extent {
            width: self.x,
            height: self.y,
            depth,
        }
    }
}

impl<T, Rhs> Div<Rhs> for Coord<T>
where
    T: Div<Rhs, Output = T>,
    Rhs: Copy,
{
    type Output = Self;

    fn div(self, rhs: Rhs) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl<T, Rhs> DivAssign<Rhs> for Coord<T>
where
    T: DivAssign<Rhs>,
    Rhs: Copy,
{
    fn div_assign(&mut self, rhs: Rhs) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl<T, U> From<(T, T)> for Coord<U>
where
    T: Into<U>,
{
    fn from(val: (T, T)) -> Self {
        Self {
            x: val.0.into(),
            y: val.1.into(),
        }
    }
}

impl From<Coord<i32>> for Coord<f32> {
    fn from(val: Coord<i32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl From<Coord<u8>> for Coord<f32> {
    fn from(val: Coord<u8>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl From<Coord<u32>> for Coord<f32> {
    fn from(val: Coord<u32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl From<Coord<f32>> for Coord<u8> {
    fn from(val: Coord<f32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl From<Coord<f32>> for Coord<u32> {
    fn from(val: Coord<f32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl From<Coord<f32>> for Vec2 {
    fn from(val: Coord<f32>) -> Self {
        Self::new(val.x as _, val.y as _)
    }
}

impl From<PhysicalSize<u32>> for Coord<u32> {
    fn from(val: PhysicalSize<u32>) -> Self {
        Self {
            x: val.width,
            y: val.height,
        }
    }
}

impl From<Coord<u32>> for Extent2D {
    fn from(val: Coord<u32>) -> Self {
        Self {
            height: val.y,
            width: val.x,
        }
    }
}

impl From<Coord<i32>> for Offset {
    fn from(val: Coord<i32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
            z: 0,
        }
    }
}

impl From<Coord<u32>> for PhysicalSize<u32> {
    fn from(val: Coord<u32>) -> Self {
        Self {
            height: val.y,
            width: val.x,
        }
    }
}

impl From<Coord<u32>> for Rect {
    fn from(val: Coord<u32>) -> Self {
        Self {
            h: val.y as _,
            w: val.x as _,
            x: 0,
            y: 0,
        }
    }
}

impl From<Coord<u32>> for Coord<i32> {
    fn from(val: Coord<u32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
        }
    }
}

impl<T, Rhs> Mul<Rhs> for Coord<T>
where
    T: Mul<Rhs, Output = T>,
    Rhs: Copy,
{
    type Output = Self;

    fn mul(self, rhs: Rhs) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl<T, Rhs> MulAssign<Rhs> for Coord<T>
where
    T: MulAssign<Rhs>,
    Rhs: Copy,
{
    fn mul_assign(&mut self, rhs: Rhs) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl<T> Neg for Coord<T>
where
    T: Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}
