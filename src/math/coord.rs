use {
    gfx_hal::{image::Extent, pso::Rect, window::Extent2D},
    winit::dpi::PhysicalSize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

impl Coord<i32> {
    pub const fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Coord<u32> {
    pub const fn as_extent(self, depth: u32) -> Extent {
        Extent {
            width: self.x,
            height: self.y,
            depth,
        }
    }

    pub const fn as_rect(self) -> Rect {
        self.as_rect_xy(Self::zero())
    }

    pub const fn as_rect_xy(self, xy: Self) -> Rect {
        Rect {
            x: xy.x as _,
            y: xy.y as _,
            w: self.x as _,
            h: self.y as _,
        }
    }

    pub const fn zero() -> Self {
        Self { x: 0, y: 0 }
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

impl From<Coord<u32>> for Coord<f32> {
    fn from(val: Coord<u32>) -> Self {
        Self {
            x: val.x as _,
            y: val.y as _,
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

impl From<Coord<u32>> for PhysicalSize<u32> {
    fn from(val: Coord<u32>) -> Self {
        Self {
            height: val.y,
            width: val.x,
        }
    }
}
