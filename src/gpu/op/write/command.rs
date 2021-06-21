use {
    crate::{
        gpu::Texture2d,
        math::{vec3, CoordF, Mat4, RectF},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    std::{
        fmt::{Debug, Error, Formatter},
        iter::{once, Once},
    },
};

/// An expressive type which allows specification of individual texture writes.
///
/// Texture writes may either specify the entire source texture or a tile sub-portion. Tiles are
/// always specified using integer texel coordinates.
pub struct Command<P>
where
    P: SharedPointerKind,
{
    /// The source texture to write.
    pub src: Shared<Texture2d, P>,

    // TODO: Examples in documentation about this one!
    /// The pixel-coordinate tile region of the source texture to write.
    pub src_tile: RectF,

    // TODO: Examples in documentation about this one!
    /// The homogenous transformation matrix for this write.
    pub transform: Mat4,
}

// TODO: Add multi-sampled builder function
impl<P> Command<P>
where
    P: SharedPointerKind,
{
    /// Writes the whole source texture to the destination at the given position.
    pub fn position<D: Into<CoordF>, S: AsRef<Shared<Texture2d, P>>>(src: S, dst: D) -> Self {
        let src_tile: RectF = src.as_ref().dims().into();

        Self::tile_position(src, src_tile, dst)
    }

    /// Writes the whole source texture to the destination at the given rectangle.
    pub fn region<D: Into<RectF>, S: AsRef<Shared<Texture2d, P>>>(src: S, dst: D) -> Self {
        let src_tile = src.as_ref().dims();

        Self::tile_region(src, src_tile, dst)
    }

    /// Writes a tile area of the source texture to the destination at the given position.
    pub fn tile_position<D: Into<CoordF>, S: AsRef<Shared<Texture2d, P>>, T: Into<RectF>>(
        src: S,
        src_tile: T,
        dst: D,
    ) -> Self {
        let src_tile: RectF = src_tile.into();

        Self::tile_region(
            src,
            src_tile,
            RectF {
                dims: src_tile.dims,
                pos: dst.into(),
            },
        )
    }

    /// Writes a tile area of the source texture to the destination at the given rectangle.
    pub fn tile_region<D: Into<RectF>, S: AsRef<Shared<Texture2d, P>>, T: Into<RectF>>(
        src: S,
        src_tile: T,
        dst: D,
    ) -> Self {
        let dst = dst.into();
        // let src_dims: CoordF = src.as_ref().dims().into();

        // PERF: This section could be hand-rolled a bit? Seems very godboltable. Get info first.
        let transform = Mat4::from_translation(vec3(dst.pos.x, dst.pos.y, 0.0))
            * Mat4::from_scale(vec3(dst.dims.x, dst.dims.y, 1.0));

        Self::tile_transform(src, src_tile, transform)
    }

    /// Writes a tile area of the source texture to the destination using the given transformation
    /// matrix.
    pub fn tile_transform<S: AsRef<Shared<Texture2d, P>>, T: Into<RectF>>(
        src: S,
        src_tile: T,
        transform: Mat4,
    ) -> Self {
        Self {
            src: Shared::clone(src.as_ref()),
            src_tile: src_tile.into(),
            transform,
        }
    }

    /// Writes the whole source texture to the destination using the given transformation matrix.
    pub fn transform<S: AsRef<Shared<Texture2d, P>>>(src: S, transform: Mat4) -> Self {
        let src_tile: RectF = src.as_ref().dims().into();

        Self::tile_transform(src, src_tile, transform)
    }
}

impl<P> Clone for Command<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self {
            src: Shared::clone(&self.src),
            ..*self
        }
    }
}

impl<P> Debug for Command<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Comand")
    }
}

impl<P> IntoIterator for Command<P>
where
    P: SharedPointerKind,
{
    type Item = Command<P>;
    type IntoIter = Once<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        once(self)
    }
}
