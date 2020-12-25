use {
    super::{
        driver::Device,
        op::{ClearOp, CopyOp, DrawOp, EncodeOp, FontOp, GradientOp, WriteOp},
        pool::{Lease, Pool},
        Driver, Op, Texture2d,
    },
    crate::{
        color::AlphaColor,
        math::{Coord, CoordF, Extent},
    },
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage},
    },
    std::path::Path,
};

/// A powerful structure which allows you to combine various operations and other render
/// instances to create just about any creative effect.
pub struct Render {
    driver: Driver,
    pool: Option<Lease<Pool>>,
    target: Lease<Texture2d>,
    target_dirty: bool,
    ops: Vec<Box<dyn Op>>,
}

impl Render {
    pub(super) fn new(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        dims: Extent,
        mut pool: Lease<Pool>,
        ops: Vec<Box<dyn Op>>,
    ) -> Self {
        let fmt = Device::best_fmt(
            &driver.borrow(),
            &[Format::Rgba8Unorm, Format::Bgra8Unorm],
            ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
        )
        .unwrap();
        let target = pool.texture(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            dims,
            fmt,
            Layout::Undefined,
            Usage::SAMPLED | Usage::TRANSFER_DST | Usage::TRANSFER_SRC,
            1,
            1,
            1,
        );

        Self {
            driver: Driver::clone(driver),
            pool: Some(pool),
            target,
            target_dirty: false,
            ops,
        }
    }

    /// Clears the screen of all text and graphics.
    pub fn clear(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut ClearOp {
        let pool = self.take_pool();
        let op = ClearOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
        );

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
        let pool = self.take_pool();
        let op = CopyOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &src,
            &self.target,
        );

        self.target_dirty = true;

        self.ops.push(Box::new(op));
        self.ops
            .last_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<CopyOp>()
            .unwrap()
    }

    pub fn dims(&self) -> Extent {
        self.target.borrow().dims()
    }

    /// Draws a batch of 3D elements. There is no need to give any particular order to the individual commands and the
    /// implementation may sort and re-order them, so do not count on indices remaining the same after this call completes.
    pub fn draw(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut DrawOp {
        let pool = self.take_pool();
        let mut op = DrawOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
        );

        if self.target_dirty {
            let _ = op.with_preserve(true);
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
    pub fn encode<P: AsRef<Path>>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> &mut EncodeOp {
        let pool = self.take_pool();
        let op = EncodeOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
        );

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
        let pool = self.take_pool();
        let op = GradientOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
            [(path[0].0, path[0].1.into()), (path[1].0, path[1].1.into())],
        );

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

    fn take_pool(&mut self) -> Lease<Pool> {
        self.pool
            .take()
            .unwrap_or_else(|| self.ops.last_mut().unwrap().take_pool().unwrap())
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
        let pool = self.take_pool();
        let op = FontOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
            pos,
            color,
        );

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
    pub fn write(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> &mut WriteOp {
        let pool = self.take_pool();
        let mut op = WriteOp::new(
            #[cfg(feature = "debug-names")]
            name,
            &self.driver,
            pool,
            &self.target,
        );

        if self.target_dirty {
            let _ = op.with_preserve(true);
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
