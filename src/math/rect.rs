use {
    super::GenericCoord,
    serde::{Deserialize, Serialize},
};

// TODO: Default impl?
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Rect<D, P>
where
    D: Sized,
    P: Sized,
{
    pub dims: GenericCoord<D>,
    pub pos: GenericCoord<P>,
}

impl<D, P> Rect<D, P>
where
    D: Sized,
    P: Sized,
{
    pub const fn new(x: P, y: P, width: D, height: D) -> Self {
        Self {
            dims: GenericCoord {
                x: width,
                y: height,
            },
            pos: GenericCoord { x, y },
        }
    }
}

impl Rect<u32, i32> {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
}

impl Rect<u32, u32> {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
}

impl Rect<f32, f32> {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);
}

impl From<GenericCoord<f32>> for Rect<f32, f32> {
    fn from(val: GenericCoord<f32>) -> Self {
        Self {
            dims: val,
            pos: GenericCoord::<f32>::ZERO,
        }
    }
}

impl From<GenericCoord<i32>> for Rect<f32, f32> {
    fn from(val: GenericCoord<i32>) -> Self {
        Self {
            dims: val.into(),
            pos: GenericCoord::<f32>::ZERO,
        }
    }
}

impl From<GenericCoord<u32>> for Rect<f32, f32> {
    fn from(val: GenericCoord<u32>) -> Self {
        Self {
            dims: val.into(),
            pos: GenericCoord::<f32>::ZERO,
        }
    }
}

impl From<GenericCoord<u32>> for Rect<u32, u32> {
    fn from(val: GenericCoord<u32>) -> Self {
        Self {
            dims: val,
            pos: GenericCoord::<u32>::ZERO,
        }
    }
}

impl From<GenericCoord<u32>> for Rect<u32, i32> {
    fn from(val: GenericCoord<u32>) -> Self {
        Self {
            dims: val,
            pos: GenericCoord::<i32>::ZERO,
        }
    }
}

impl From<Rect<u32, i32>> for Rect<f32, f32> {
    fn from(val: Rect<u32, i32>) -> Self {
        Self {
            dims: val.dims.into(),
            pos: val.pos.into(),
        }
    }
}

impl From<Rect<u32, u32>> for Rect<f32, f32> {
    fn from(val: Rect<u32, u32>) -> Self {
        Self {
            dims: val.dims.into(),
            pos: val.pos.into(),
        }
    }
}
