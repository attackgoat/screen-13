use {
    super::{
        driver::PhysicalDevice,
        op::{
            ClearOp, Command, CopyOp, DrawOp, EncodeOp, Font, FontOp, GradientOp, Write, WriteMode,
            WriteOp,
        },
        pool::{Lease, Pool},
        Driver, Op, Texture2d, TextureRef,
    },
    crate::{
        camera::Camera,
        color::{AlphaColor, Color},
        math::{Area, Coord, CoordF, Extent},
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
    pool: Lease<Pool>,
    target: Lease<Texture2d>,
    target_dirty: bool,
    ops: Vec<Box<dyn Op>>,
}

impl Render {
    pub(super) fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        mut pool: Lease<Pool>,
        dims: Extent,
        ops: Vec<Box<dyn Op>>,
    ) -> Self {
        let fmt = driver
            .borrow()
            .best_fmt(
                &[Format::Rgba8Unorm, Format::Bgra8Unorm],
                ImageFeature::COLOR_ATTACHMENT | ImageFeature::SAMPLED,
            )
            .unwrap();
        let target = pool.texture(
            #[cfg(debug_assertions)]
            name,
            &driver,
            dims,
            fmt,
            Layout::Undefined,
            Usage::SAMPLED | Usage::TRANSFER_DST | Usage::TRANSFER_SRC,
            1,
            1,
            1,
        );

        Self {
            driver,
            pool,
            target,
            target_dirty: false,
            ops,
        }
    }

    /// Clears the screen of all text and graphics.
    pub fn clear(&mut self, #[cfg(debug_assertions)] name: &str, color: Color) {
        let format = self.target.borrow().format();
        let mut op = ClearOp::new(
            #[cfg(debug_assertions)]
            name,
            &self.driver,
            &mut self.pool,
            &self.target,
        );
        op.with_clear_value(color.swizzle(format));
        self.ops.push(Box::new(op.record()));
        self.target_dirty = true;
    }

    /// Copies the given texture onto this Render. The implementation uses a copy operation
    /// and is more efficient than `write` when there is no blending or fractional pixels.
    pub fn copy(&mut self, #[cfg(debug_assertions)] name: &str, src: &Texture2d) {
        self.ops.push(Box::new(
            CopyOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.driver,
                &mut self.pool,
                &src,
                &self.target,
            )
            .record(),
        ));
        self.target_dirty = true;
    }

    /// Copies a region of the given texture onto this Render at `dst` coordinates. The
    /// implementation uses a copy operation and is more efficient than `write` when there
    /// is no blending or fractional pixels.
    pub fn copy_region(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        src: &Texture2d,
        src_region: Area,
        dst: Extent,
    ) {
        let mut op = CopyOp::new(
            #[cfg(debug_assertions)]
            name,
            &self.driver,
            &mut self.pool,
            &src,
            &self.target,
        );
        op.with_region(src_region, dst);
        self.ops.push(Box::new(op.record()));
        self.target_dirty = true;
    }

    pub fn dims(&self) -> Extent {
        self.target.borrow().dims()
    }

    /// Draws a batch of 3D elements. There is no need to give any particular order to the individual commands and the
    /// implementation may sort and re-order them, so do not count on indices remaining the same after this call completes.
    pub fn draw(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        camera: &impl Camera,
        cmds: &mut [Command],
    ) {
        let mut op = DrawOp::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&self.driver),
            &mut self.pool,
            &self.target,
        );

        if self.target_dirty {
            op.with_preserve();
        }

        self.ops.push(Box::new(op.record(camera, cmds)));
        self.target_dirty = true;
    }

    /// Saves this Render as a JPEG file at the given path.
    pub fn encode<P: AsRef<Path>>(&mut self, #[cfg(debug_assertions)] name: &str, path: P) {
        self.ops.push(Box::new(
            EncodeOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.driver,
                &mut self.pool,
                TextureRef::clone(&self.target),
            )
            .record(path),
        ));
    }

    /// Draws a linear gradient on this Render using the given path.
    /// TODO: Specialize for radial too?
    pub fn gradient<C>(&mut self, #[cfg(debug_assertions)] name: &str, path: [(Coord, C); 2])
    where
        C: Copy + Into<AlphaColor>,
    {
        self.ops.push(Box::new(
            GradientOp::new(
                #[cfg(debug_assertions)]
                name,
                Driver::clone(&self.driver),
                &mut self.pool,
                &self.target,
                [(path[0].0, path[0].1.into()), (path[1].0, path[1].1.into())],
            )
            .record(),
        ));
        self.target_dirty = true;
    }

    pub(crate) fn resolve(self) -> (Lease<Texture2d>, Vec<Box<dyn Op>>) {
        (self.target, self.ops)
    }

    /// Draws bitmapped text on this Render using the given details.
    /// TODO: Accept a list of font/color/text/pos combos so we can batch many at once?
    pub fn text<C, P>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        font: &Font,
        text: &str,
        pos: P,
        color: C,
    ) where
        C: Into<AlphaColor>,
        P: Into<CoordF>,
    {
        self.ops.push(Box::new(
            FontOp::new(
                #[cfg(debug_assertions)]
                name,
                Driver::clone(&self.driver),
                &mut self.pool,
                TextureRef::clone(&self.target),
                pos,
                color,
            )
            .record(font, text),
        ));
        self.target_dirty = true;
    }

    /// Draws bitmapped text on this Render using the given details.
    /// TODO: Accept a list of font/color/text/pos combos so we can batch many at once?
    pub fn text_outline<C1, C2, P>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        font: &Font,
        text: &str,
        pos: P,
        color: C1,
        outline_color: C2,
    ) where
        C1: Into<AlphaColor>,
        C2: Into<AlphaColor>,
        P: Into<CoordF>,
    {
        let mut op = FontOp::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&self.driver),
            &mut self.pool,
            TextureRef::clone(&self.target),
            pos,
            color,
        );
        op.with_outline_color(outline_color);
        self.ops.push(Box::new(op.record(font, text)));
        self.target_dirty = true;
    }

    /// Draws the given texture writes onto this Render. Note that the given texture writes will all be applied at once and there
    /// is no 'layering' of the individual writes going on - so if you need blending between writes you must submit a new batch.
    pub fn write(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        mode: WriteMode,
        writes: &mut [Write],
    ) {
        let mut op = WriteOp::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&self.driver),
            &mut self.pool,
            TextureRef::clone(&self.target),
            mode,
        );

        if self.target_dirty {
            op.with_preserve();
        }

        self.ops.push(Box::new(op.record(writes)));
        self.target_dirty = true;
    }
}
