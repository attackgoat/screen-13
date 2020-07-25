use {
    super::{
        op::{
            draw::{Command, DrawOp},
            ClearOp, CopyOp, EncodeOp, Font, FontOp, GradientOp, Op, Write, WriteMode, WriteOp,
        },
        pool::Lease,
        Image, PoolRef, Texture2d, TextureRef,
    },
    crate::{
        camera::Camera,
        color::{AlphaColor, Color},
        math::{vec3, Area, Coord, CoordF, Extent, Mat4},
    },
    gfx_hal::{
        format::Format,
        image::{Access, Layout, Tiling, Usage},
        pso::PipelineStage,
    },
    std::{collections::VecDeque, ops::Deref, path::Path},
};

/// Holds a potentially in-progress GPU operation. This opaque type represents the work
/// being done by the GPU and will cause the GPU to stall if dropped prematurely. You
/// must give the GPU time to finish this work, so keep it in a queue of some sort for
/// a few frames.
pub struct Operation(Box<dyn Op>);

/// A powerful structure which allows you to combine various operations and other render
/// instances to create just about any creative effect.
pub struct Render {
    dims: Extent,
    format: Format,
    pool: PoolRef,
    target: Lease<Texture2d>,
    ops: VecDeque<Operation>, // TODO: Should be just a vec?
}

impl Render {
    pub(super) fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        dims: Extent,
        format: Format,
    ) -> Self {
        Self {
            dims,
            format,
            pool: PoolRef::clone(pool),
            target: pool.borrow_mut().texture(
                #[cfg(debug_assertions)]
                name,
                dims,
                Tiling::Optimal,
                format,
                Layout::Undefined,
                Usage::SAMPLED | Usage::TRANSFER_DST | Usage::TRANSFER_SRC,
                1,
                1,
                1,
            ),
            ops: Default::default(),
        }
    }

    /// Clears the screen of all text and graphics.
    pub fn clear(&mut self, color: Color) {
        self.ops.push_front(Operation(Box::new(
            ClearOp::new(&mut self.pool.borrow_mut(), &self.target)
                .with_clear_value(color.swizzle(self.format))
                .record(),
        )));
    }

    /// Copies the given texture onto this Render. The implementation uses a copy operation
    /// and is more efficient than `write` when there is no blending or fractional pixels.
    pub fn copy(&mut self, src: &Texture2d) {
        self.ops.push_front(Operation(Box::new(
            CopyOp::new(&self.pool, &src, &self.target).record(),
        )));
    }

    /// Copies a region of the given texture onto this Render at `dst` coordinates. The
    /// implementation uses a copy operation and is more efficient than `write` when there
    /// is no blending or fractional pixels.
    pub fn copy_region(&mut self, src: &Texture2d, src_region: Area, dst: Extent) {
        self.ops.push_front(Operation(Box::new(
            CopyOp::new(&self.pool, &src, &self.target)
                .with_region(src_region, dst)
                .record(),
        )));
    }

    pub fn dims(&self) -> Extent {
        self.dims
    }

    /// Draws a batch of 3D elements. There is no need to give any particular order to the individual commands and the
    /// implemtation may sort and re-order them, so do not count on indices remaining the same after this call completes.
    pub fn draw<'c>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        camera: &impl Camera,
        cmds: &'c mut [Command<'c>],
    ) {
        self.ops.push_front(Operation(Box::new(
            DrawOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
            )
            .record(camera, cmds),
        )));
    }

    /// Saves this Render as a JPEG file at the given path.
    pub fn encode<P: AsRef<Path>>(&mut self, #[cfg(debug_assertions)] name: &str, path: P) {
        self.ops.push_front(Operation(Box::new(
            EncodeOp::new(
                #[cfg(debug_assertions)]
                name,
                &mut self.pool.borrow_mut(),
                TextureRef::clone(&self.target),
            )
            .record(path),
        )))
    }

    // TODO: Kill with resolve!
    pub fn extend_ops<O>(&mut self, ops: O)
    where
        O: IntoIterator<Item = Operation>,
    {
        // Best practice?
        // // let (render, mut ops) = render.resolve();
        // // if !ops.is_empty() {
        // //     self.extend_ops(ops.drain(ops.len() - 1..=0));
        // // }

        ops.into_iter().for_each(|op| self.ops.push_front(op));
    }

    /// Draws a linear gradient on this Render using the given path.
    /// TODO: Specialize for radial too?
    pub fn gradient<C>(&mut self, #[cfg(debug_assertions)] name: &str, path: [(Coord, C); 2])
    where
        C: Copy + Into<AlphaColor>,
    {
        self.ops.push_front(Operation(Box::new(
            GradientOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
                [(path[0].0, path[0].1.into()), (path[1].0, path[1].1.into())],
            )
            .record(),
        )));
    }

    /// This crate-only helper function is a specialization of the write function which writes this
    /// Render onto the given destination texture, which should be a swapchain backbuffer image.
    /// The image is stretched unfiltered so there will be pixel doubling artifacts unless the
    /// dimensions are equal.
    pub(crate) fn present(&mut self, #[cfg(debug_assertions)] name: &str, dst: &TextureRef<Image>) {
        let dst_dims: CoordF = dst.borrow().dims().into();
        let src_dims: CoordF = self.dims.into();

        // Scale is the larger of either X or Y when stretching to cover all four sides
        let scale_x = dst_dims.x / src_dims.x;
        let scale_y = dst_dims.y / src_dims.y;
        let scale = scale_x.max(scale_y);

        // Transform is scaled and centered on the dst texture
        let transform = Mat4::from_scale(vec3(
            src_dims.x * scale / dst_dims.x * 2.0,
            src_dims.y * scale / dst_dims.y * 2.0,
            1.0,
        )) * Mat4::from_translation(vec3(-0.5, -0.5, 0.0));

        // Note: This does not preserve the existing contents of `dst`
        self.ops.push_front(Operation(Box::new(
            WriteOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                dst,
                WriteMode::Texture,
            )
            .with_layout(
                Layout::Present,
                PipelineStage::BOTTOM_OF_PIPE,
                Access::empty(),
            )
            .record(&mut [Write::transform(&*self.target, transform)]),
        )));
    }

    /// Renders this instance to a texture; it is available once all the ops are waited on
    /// TODO: Find a way to not offer this signature and still have R2T capability?
    pub fn resolve(mut self) -> (impl Deref<Target = Texture2d>, Vec<Operation>) {
        (self.target, self.ops.drain(..).collect::<Vec<_>>())
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
        self.ops.push_front(Operation(Box::new(
            FontOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
                pos,
                color,
            )
            .record(font, text),
        )));
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
        self.ops.push_front(Operation(Box::new(
            FontOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
                pos,
                color,
            )
            .with_outline_color(outline_color)
            .record(font, text),
        )));
    }

    /// Draws the given texture writes onto this Render. Note that the given texture writes will all be applied at once and there
    /// is no 'layering' of the individual writes going on - so if you need blending between writes you must submit a new batch.
    pub fn write(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        mode: WriteMode,
        writes: &mut [Write],
    ) {
        self.ops.push_front(Operation(Box::new(
            WriteOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
                mode,
            )
            .with_preserve()
            .record(writes),
        )));
    }
}
