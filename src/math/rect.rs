use super::GenericCoord;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rect<D, P>
where
    D: Copy,
    P: Copy,
{
    pub dims: GenericCoord<D>,
    pub pos: GenericCoord<P>,
}

impl<D, P> Rect<D, P>
where
    D: Copy,
    P: Copy,
{
    pub fn new(x: P, y: P, width: D, height: D) -> Self {
        Self {
            dims: GenericCoord {
                x: width,
                y: height,
            },
            pos: GenericCoord { x, y },
        }
    }
}
