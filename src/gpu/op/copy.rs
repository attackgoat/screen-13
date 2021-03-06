use {
    super::Op,
    crate::{
        gpu::{
            driver::{CommandPool, Fence},
            pool::{Lease, Pool},
            queue_mut, Texture2d,
        },
        math::{Area, Coord, Extent},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::Queue as _,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        iter::{empty, once},
    },
};

// TODO: Add automatic submit-skipping by adding support for the "auto-cull" feature?
// TODO: This should use the blit command when possible
/// A container of graphics types for efficiently copying textures between each other.
///
/// _NOTE:_ Regions submitted for copy operations do not need to be valid regions for
/// the given textures; they can overlap or fall off the edges.
pub struct CopyOp<P>
where
    P: 'static + SharedPointerKind,
{
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    dst: Shared<Texture2d, P>,
    dst_offset: Extent,
    fence: Lease<Fence, P>,
    pool: Option<Lease<Pool<P>, P>>,
    region: Extent,
    src: Shared<Texture2d, P>,
    src_offset: Extent,
}

impl<P> CopyOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        src: &Shared<Texture2d, P>,
        dst: &Shared<Texture2d, P>,
    ) -> Self {
        let (cmd_buf, cmd_pool, fence) = {
            let mut cmd_pool = pool.cmd_pool();
            let fence = pool.fence(
                #[cfg(feature = "debug-names")]
                name,
            );

            let cmd_buf = cmd_pool.allocate_one(Level::Primary);

            (cmd_buf, cmd_pool, fence)
        };

        Self {
            cmd_buf,
            cmd_pool,
            dst: Shared::clone(dst),
            dst_offset: Extent::ZERO,
            fence,
            pool: Some(pool),
            region: src.dims(),
            src: Shared::clone(src),
            src_offset: Extent::ZERO,
        }
    }

    /// Specifies an identically-sized area of the source and destination to copy, and the position on the
    /// destination where the data will go.
    #[must_use]
    pub fn with_region(&mut self, src_region: Area, dst: Extent) -> &mut Self {
        self.dst_offset = dst;
        self.region = src_region.dims;
        self.src_offset = src_region.pos;
        self
    }

    /// Submits the given copy for hardware processing.
    pub fn record(&mut self) {
        unsafe {
            self.submit();
        }
    }

    unsafe fn submit(&mut self) {
        trace!("submit");

        let dst_offset: Coord = self.dst_offset.into();
        let dst_offset = dst_offset.into();
        let src_offset: Coord = self.src_offset.into();
        let src_offset = src_offset.into();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy src image to dst image
        self.src.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        self.dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            self.src.as_ref(),
            Layout::TransferSrcOptimal,
            self.dst.as_ref(),
            Layout::TransferDstOptimal,
            once(ImageCopy {
                dst_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                dst_offset,
                extent: self.region.as_extent_depth(1),
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset,
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        queue_mut().submit(once(&self.cmd_buf), empty(), empty(), Some(&mut self.fence));
    }
}

impl<P> Drop for CopyOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait();
        }
    }
}

impl<P> Op<P> for CopyOp<P>
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn is_complete(&self) -> bool {
        Fence::status(&self.fence)
    }

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P> {
        self.pool.take().unwrap()
    }

    unsafe fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
