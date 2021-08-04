use {
    super::{
        op::{
            clear::ClearOp, copy::CopyOp, draw::DrawOp, encode::EncodeOp, gradient::GradientOp,
            text::TextOp, write::WriteOp, Op,
        },
        pool::{Lease, Pool},
        Texture2d,
    },
    crate::{
        color::AlphaColor,
        math::{Coord, Extent},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        format::{Format, ImageFeature},
        image::{Layout, Usage},
    },
    std::{
        fmt::{Debug, Error, Formatter},
        vec::Drain,
    },
};

/// A powerful structure which allows you to combine various operations and other render
/// instances to create just about any creative effect.
pub struct Render<P>
where
    P: 'static + SharedPointerKind,
{
    pool: Option<Lease<Pool<P>, P>>,
    target: Lease<Shared<Texture2d, P>, P>,
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
            Usage::SAMPLED | Usage::TRANSFER_DST | Usage::TRANSFER_SRC,
            1,
            1,
            1,
        );

        Self {
            pool: Some(pool),
            target,
            target_dirty: false,
            ops: Default::default(),
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
        src: &Shared<Texture2d, P>,
    ) -> &mut CopyOp<P> {
        let op = unsafe {
            let pool = self.take_pool();
            CopyOp::new(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                src,
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
        self.target.dims()
    }

    /// Draws a batch of 3D elements.
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

    pub(crate) fn drain_ops(&mut self) -> Drain<'_, Box<dyn Op<P>>> {
        self.ops.drain(..)
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

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool
            .take()
            .unwrap_or_else(|| self.ops.last_mut().unwrap().take_pool())
    }

    /// Draws text on this Render using bitmapped or vector fonts.
    pub fn text(&mut self, #[cfg(feature = "debug-names")] name: &str) -> &mut TextOp<P> {
        let op = unsafe {
            let pool = self.take_pool();
            TextOp::new(
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
            .downcast_mut::<TextOp<P>>()
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

impl<P> AsRef<Shared<Texture2d, P>> for Render<P>
where
    P: SharedPointerKind,
{
    fn as_ref(&self) -> &Shared<Texture2d, P> {
        &self.target
    }
}

impl<P> AsRef<Texture2d> for Render<P>
where
    P: SharedPointerKind,
{
    fn as_ref(&self) -> &Texture2d {
        &**self.target
    }
}

impl<P> Debug for Render<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Render")
    }
}

impl<P> Drop for Render<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if self.ops.is_empty() {
            return;
        }

        // Store any un-presented ops in the pool for now
        let mut pool = unsafe { self.take_pool() };
        for op in self.ops.drain(..) {
            pool.ops.push_front(op);
        }
    }
}
