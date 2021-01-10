use {
    super::{
        op::{
            clear::ClearOp, copy::CopyOp, draw::DrawOp, encode::EncodeOp, font::FontOp,
            gradient::GradientOp, write::WriteOp, Op,
        },
        pool::{Lease, Pool},
        Texture2d,
    },
    crate::{
        color::AlphaColor,
        math::{Coord, CoordF, Extent},
    },
    archery::SharedPointerKind,
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage},
    },
};

/// A powerful structure which allows you to combine various operations and other render
/// instances to create just about any creative effect.
pub struct Render<P>
where
    P: 'static + SharedPointerKind,
{
    pool: Option<Lease<Pool<P>, P>>,
    target: Lease<Texture2d, P>,
    target_dirty: bool,
    ops: Vec<Box<dyn Op<P>>>,
}

impl<P> Render<P>
where
    P: 'static + SharedPointerKind,
{
    pub(super) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        dims: Extent,
        mut pool: Lease<Pool<P>, P>,
        ops: Vec<Box<dyn Op<P>>>,
    ) -> Self {
        let fmt = pool
            .best_fmt(
                &[Format::Rgba8Unorm, Format::Bgra8Unorm],
                ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
        let target = pool.texture(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            fmt,
            Layout::Undefined,
            Usage::SAMPLED,
            1,
            1,
            1,
        );

        Self {
            pool: Some(pool),
            target,
            target_dirty: false,
            ops,
        }
    }

    /// Clears the screen of all text and graphics.
    pub fn clear(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut ClearOp<P> {
        let op = unsafe {
            let pool = self.take_pool();
            ClearOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
            )
        };

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<ClearOp<P>>()
            .unwrap()
    }

    /// Copies the given texture onto this Render.
    ///
    /// The implementation uses a copy operation and is more efficient than `write` when there is no
    /// blending or fractional pixels.
    pub fn copy(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        src: &Texture2d,
    ) -> &mut CopyOp<P> {
        let op = unsafe {
            let pool = self.take_pool();
            CopyOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &src,
                &self.target,
            )
        };

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<CopyOp<P>>()
            .unwrap()
    }

    /// Gets the dimensions, in pixels, of this `Render`.
    pub fn dims(&self) -> Extent {
        self.target.borrow().dims()
    }

    /// Draws a batch of 3D elements.
    ///
    /// **_NOTE:_** The implementation may re-order the provided draws, so do not rely on existing
    /// indices after this call completes.
    ///
    /// **_NOTE:_** Not fully implemented yet
    pub fn draw(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut DrawOp<P> {
        let mut op = unsafe {
            let pool = self.take_pool();
            DrawOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
            )
        };

        if self.target_dirty {
            let _ = op.with_preserve();
        }

        self.target_dirty = true;
        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<DrawOp<P>>()
            .unwrap()
    }

    /// Saves this Render as a JPEG file at the given path.
    pub fn encode(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut EncodeOp<P> {
        let op = unsafe {
            let pool = self.take_pool();
            EncodeOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
            )
        };

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<EncodeOp<P>>()
            .unwrap()
    }

    // TODO: Specialize for radial too?
    /// Draws a linear gradient on this Render using the given path.
    ///
    /// **_NOTE:_** Not fully implemented yet
    pub fn gradient<C>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        path: [(Coord, C); 2],
    ) -> &mut GradientOp<P>
    where
        C: Copy + Into<AlphaColor>,
    {
        let op = unsafe {
            let pool = self.take_pool();
            GradientOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
                [(path[0].0, path[0].1.into()), (path[1].0, path[1].1.into())],
            )
        };

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<GradientOp<P>>()
            .unwrap()
    }

    // TODO: Remove this function, allow using Renders as textures naturually
    /// This is going to change soon! Possibly just go away and be used implicitly without this
    /// function.
    #[allow(clippy::type_complexity)]
    pub(crate) fn resolve(self) -> (Lease<Texture2d, P>, Vec<Box<dyn Op<P>>>) {
        (self.target, self.ops)
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool
            .take()
            .unwrap_or_else(|| self.ops.last_mut().unwrap().take_pool())
    }

    // TODO: Accept a list of font/color/text/pos combos so we can batch many at once?
    /// Draws bitmapped text on this Render using the given details.
    pub fn text<C, O>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pos: O,
        color: C,
    ) -> &mut FontOp<P>
    where
        C: Into<AlphaColor>,
        O: Into<CoordF>,
    {
        let op = unsafe {
            let pool = self.take_pool();
            FontOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
                pos,
                color,
            )
        };

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<FontOp<P>>()
            .unwrap()
    }

    /// Draws the given texture writes onto this Render.
    ///
    /// **_Note:_** The given texture writes will all be applied at once without 'layering' of the
    /// individual writes. If you need blending between writes you must submit multiple batches.
    pub fn write(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut WriteOp<P> {
        let mut op = unsafe {
            let pool = self.take_pool();
            WriteOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                &self.target,
            )
        };

        if self.target_dirty {
            let _ = op.with_preserve();
        }

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<WriteOp<P>>()
            .unwrap()
    }
}
