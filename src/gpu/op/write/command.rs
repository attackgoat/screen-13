use {
    crate::{
        gpu::Texture2d,
        math::{vec3, Area, CoordF, Mat4, RectF},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::fmt::{Debug, Error, Formatter},
};

/// An expressive type which allows specification of individual texture writes. Texture writes may either specify the
/// entire source texture or a tile sub-portion. Tiles are always specified using integer texel coordinates.
pub struct Command<P>
where
    P: SharedPointerKind,
{
    pub src: Shared<Texture2d, P>,
    pub src_region: Area,
    pub transform: Mat4,
}

// TODO: Add multi-sampled builder function
impl<P> Command<P>
where
    P: SharedPointerKind,
{
    /// Writes the whole source texture to the destination at the given position.
    pub fn position<D: Into<CoordF>, S: AsRef<Shared<Texture2d, P>>>(src: S, dst: D) -> Self {
        let dims = src.as_ref().dims().into();

        Self::tile_position(src, dims, dst)
    }

    /// Writes the whole source texture to the destination at the given rectangle.
    pub fn region<D: Into<RectF>, S: AsRef<Shared<Texture2d, P>>>(src: S, dst: D) -> Self {
        let dims = src.as_ref().dims().into();

        Self::tile_region(src, dims, dst)
    }

    /// Writes a tile area of the source texture to the destination at the given position.
    pub fn tile_position<D: Into<CoordF>, S: AsRef<Shared<Texture2d, P>>>(
        src: S,
        src_tile: Area,
        dst: D,
    ) -> Self {
        let dims = src.as_ref().dims().into();

        Self::tile_region(
            src,
            src_tile,
            RectF {
                dims,
                pos: dst.into(),
            },
        )
    }

    /// Writes a tile area of the source texture to the destination at the given rectangle.
    pub fn tile_region<D: Into<RectF>, S: AsRef<Shared<Texture2d, P>>>(
        src: S,
        src_tile: Area,
        dst: D,
    ) -> Self {
        let dst = dst.into();
        let src_dims: CoordF = src.as_ref().dims().into();

        // PERF: This section could be hand-rolled a bit? Seems very godboltable. Get info first.
        let dst_transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(
                dst.dims.x * 2.0 / src_dims.x,
                dst.dims.y * 2.0 / src_dims.y,
                1.0,
            ))
            * Mat4::from_translation(vec3(dst.pos.x / dst.dims.x, dst.pos.y / dst.dims.y, 0.0));

        Self::tile_transform(src, src_tile, dst_transform)
    }

    /// Writes a tile area of the source texture to the destination using the given transformation matrix.
    pub fn tile_transform<S: AsRef<Shared<Texture2d, P>>>(
        src: S,
        src_tile: Area,
        dst: Mat4,
    ) -> Self {
        Self {
            src: Shared::clone(src.as_ref()),
            src_region: src_tile,
            transform: dst,
        }
    }

    /// Writes the whole source texture to the destination using the given transformation matrix.
    pub fn transform<S: AsRef<Shared<Texture2d, P>>>(src: S, dst: Mat4) -> Self {
        let dims = src.as_ref().dims().into();

        Self::tile_transform(src, dims, dst)
    }
}

impl<P> Clone for Command<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self {
            src: Shared::clone(&self.src),
            src_region: self.src_region,
            transform: self.transform,
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
