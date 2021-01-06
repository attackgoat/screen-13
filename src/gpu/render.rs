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
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage},
    },
};

/// A powerful structure which allows you to combine various operations and other render
/// instances to create just about any creative effect.
pub struct Render {
    pool: Option<Lease<Pool>>,
    target: Lease<Texture2d>,
    target_dirty: bool,
    ops: Vec<Box<dyn Op>>,
}

impl Render {
    pub(super) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        dims: Extent,
        mut pool: Lease<Pool>,
        ops: Vec<Box<dyn Op>>,
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
    pub fn clear(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut ClearOp {
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
            .downcast_mut::<ClearOp>()
            .unwrap()
    }

    /// Copies the given texture onto this Render. The implementation uses a copy operation
    /// and is more efficient than `write` when there is no blending or fractional pixels.
    pub fn copy(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        src: &Texture2d,
    ) -> &mut CopyOp {
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
            .downcast_mut::<CopyOp>()
            .unwrap()
    }

    /// Gets the dimensions, in pixels, of this `Render`.
    pub fn dims(&self) -> Extent {
        self.target.borrow().dims()
    }

    /// Draws a batch of 3D elements. There is no need to give any particular order to the individual commands and the
    /// implementation may sort and re-order them, so do not count on indices remaining the same after this call completes.
    pub fn draw(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut DrawOp {
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
            .downcast_mut::<DrawOp>()
            .unwrap()
    }

    /// Saves this Render as a JPEG file at the given path.
    pub fn encode(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut EncodeOp {
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
            .downcast_mut::<EncodeOp>()
            .unwrap()
    }

    /// Draws a linear gradient on this Render using the given path.
    /// TODO: Specialize for radial too?
    pub fn gradient<C>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        path: [(Coord, C); 2],
    ) -> &mut EncodeOp
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
            .downcast_mut::<EncodeOp>()
            .unwrap()
    }

    pub(crate) fn resolve(self) -> (Lease<Texture2d>, Vec<Box<dyn Op>>) {
        (self.target, self.ops)
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool> {
        self.pool
            .take()
            .unwrap_or_else(|| self.ops.last_mut().unwrap().take_pool())
    }

    /// Draws bitmapped text on this Render using the given details.
    /// TODO: Accept a list of font/color/text/pos combos so we can batch many at once?
    pub fn text<C, P>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pos: P,
        color: C,
    ) -> &mut FontOp
    where
        C: Into<AlphaColor>,
        P: Into<CoordF>,
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
            .downcast_mut::<FontOp>()
            .unwrap()
    }

    /// Draws the given texture writes onto this Render. Note that the given texture writes will all be applied at once and there
    /// is no 'layering' of the individual writes going on - so if you need blending between writes you must submit a new batch.
    pub fn write(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut WriteOp {
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
            .downcast_mut::<WriteOp>()
            .unwrap()
    }
}
