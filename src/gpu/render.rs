use {
    super::{
        op::{
            ClearOp, Command, CopyOp, DrawOp, EncodeOp, Font, FontOp, GradientOp, Op, WriteMode,
            WriteOp,
        },
        pool::Lease,
        Image, PoolRef, Texture2d, TextureRef,
    },
    crate::{
        camera::Camera,
        color::{AlphaColor, Color},
        math::{vec3, Coord, CoordF, Extent, Mat4},
    },
    gfx_hal::{
        format::Format,
        image::{Access, Layout, Tiling, Usage},
        pso::PipelineStage,
    },
    std::{collections::VecDeque, path::Path},
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

    // TODO: Needs multiple version, one with src/dst rect
    pub fn copy(&mut self, src: &Texture2d) {
        self.ops.push_front(Operation(Box::new(
            CopyOp::new(&self.pool, &src, &self.target).record(),
        )));
    }

    pub fn dims(&self) -> Extent {
        self.dims
    }

    pub fn draw<'c>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        camera: &impl Camera,
        cmds: &mut [Command<'c>],
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

    pub(crate) fn present(&mut self, #[cfg(debug_assertions)] name: &str, dst: &TextureRef<Image>) {
        let dst_dims: CoordF = dst.borrow().dims().into();
        let src_dims: CoordF = self.dims.into();
        let scale_x = dst_dims.x / src_dims.x;
        let scale_y = dst_dims.y / src_dims.y;
        let scale = scale_x.max(scale_y);
        self.ops.push_front(Operation(Box::new(
            WriteOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                self.target.as_ref(),
                dst,
            )
            .with_mode(WriteMode::Texture)
            .with_transform(
                Mat4::from_scale(vec3(
                    src_dims.x * scale / dst_dims.x * 2.0,
                    src_dims.y * scale / dst_dims.y * 2.0,
                    1.0,
                )) * Mat4::from_translation(vec3(-0.5, -0.5, 0.0)),
            )
            .with_dst_layout(
                Layout::Present,
                PipelineStage::BOTTOM_OF_PIPE,
                Access::empty(),
            )
            .record(),
        )));
    }

    /// Renders this instance to a texture; it is available once all the ops are waited on
    /// TODO: Find a way to not offer this signature and still have R2T capability?
    pub fn resolve(mut self) -> (impl AsRef<Texture2d>, Vec<Operation>) {
        (self.target, self.ops.drain(..).collect::<Vec<_>>())
    }

    pub fn text<C>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        font: &Font,
        text: &str,
        pos: Coord,
        color: C,
    ) where
        C: Into<AlphaColor>,
    {
        self.ops.push_front(Operation(Box::new(
            FontOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
            )
            .with_glyph_color(color)
            .with_pos(pos)
            .record(font, text),
        )));
    }

    pub fn text_outline<C, O>(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        font: &Font,
        text: &str,
        pos: Coord,
        glyph_color: C,
        outline_color: O,
    ) where
        C: Into<AlphaColor>,
        O: Into<AlphaColor>,
    {
        self.ops.push_front(Operation(Box::new(
            FontOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                &self.target,
            )
            .with_glyph_color(glyph_color)
            .with_outline_color(outline_color)
            .with_pos(pos)
            .record(font, text),
        )));
    }

    pub fn write(&mut self, #[cfg(debug_assertions)] name: &str, src: &Texture2d, pos: Coord) {
        let dims = src.borrow().dims();
        self.write_dims(
            #[cfg(debug_assertions)]
            name,
            src,
            pos,
            dims,
        );
    }

    pub fn write_dims(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        src: &Texture2d,
        pos: Coord,
        dims: Extent,
    ) {
        self.write_mode(
            #[cfg(debug_assertions)]
            name,
            src,
            pos,
            dims,
            WriteMode::Texture,
        );
    }

    pub fn write_mode(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        src: &Texture2d,
        pos: Coord,
        dims: Extent,
        mode: WriteMode,
    ) {
        let transform = Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
            * Mat4::from_scale(vec3(
                dims.x as f32 * 2.0 / self.dims.x as f32,
                dims.y as f32 * 2.0 / self.dims.y as f32,
                1.0,
            ))
            * Mat4::from_translation(vec3(
                pos.x as f32 / dims.x as f32,
                pos.y as f32 / dims.y as f32,
                0.0,
            ));

        self.ops.push_front(Operation(Box::new(
            WriteOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                src,
                &self.target,
            )
            .with_mode(mode)
            .with_transform(transform)
            .with_preserve_dst()
            .record(),
        )));
    }

    pub fn write_transform(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        src: &Texture2d,
        transform: Mat4,
        mode: WriteMode,
    ) {
        self.ops.push_front(Operation(Box::new({
            let mut op = WriteOp::new(
                #[cfg(debug_assertions)]
                name,
                &self.pool,
                src,
                &self.target,
            )
            .with_mode(mode)
            .with_transform(transform);

            // The write-texture mode does not preserve the destination beneath this operation
            if mode != WriteMode::Texture {
                op = op.with_preserve_dst();
            }

            op.record()
        })));
    }
}
