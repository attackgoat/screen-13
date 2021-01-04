use {
    super::Op,
    crate::{
        gpu::{
            driver::{CommandPool, Device, Driver, Fence},
            pool::{Lease, Pool},
            Texture2d,
        },
        math::{Area, Coord, Extent},
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        any::Any,
        iter::{empty, once},
    },
};

// TODO: This should use the blit command when possible
/// A container of graphics types for efficiently copying textures between each other.
///
/// _NOTE:_ Regions submitted for copy operations do not need to be valid regions for
/// the given textures; they can overlap or fall off the edges.
pub struct CopyOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    device: Device,
    dst: Texture2d,
    dst_offset: Extent,
    fence: Lease<Fence>,
    pool: Option<Lease<Pool>>,
    region: Extent,
    src: Texture2d,
    src_offset: Extent,
}

impl CopyOp {
    #[must_use]
    pub(crate) fn new(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        mut pool: Lease<Pool>,
        src: &Texture2d,
        dst: &Texture2d,
    ) -> Self {
        let (cmd_buf, cmd_pool, fence) = {
            let family = Device::queue_family(&driver.borrow());
            let mut cmd_pool = pool.cmd_pool(driver, family);
            let fence = pool.fence(
                #[cfg(feature = "debug-names")]
                name,
                driver,
            );

            let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };

            (cmd_buf, cmd_pool, fence)
        };

        Self {
            cmd_buf,
            cmd_pool,
            device: Device::clone(driver),
            dst: Texture2d::clone(dst),
            dst_offset: Extent::ZERO,
            fence,
            pool: Some(pool),
            region: src.borrow().dims(),
            src: Texture2d::clone(src),
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

        let mut device = self.driver.borrow_mut();
        let mut src = self.src.borrow_mut();
        let mut dst = self.dst.borrow_mut();
        let dst_offset: Coord = self.dst_offset.into();
        let dst_offset = dst_offset.into();
        let src_offset: Coord = self.src_offset.into();
        let src_offset = src_offset.into();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy src image to dst image
        src.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_READ,
        );
        dst.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_image(
            src.as_ref(),
            Layout::TransferSrcOptimal,
            dst.as_ref(),
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
        Device::queue_mut(&mut device).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }
}

impl Drop for CopyOp {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for CopyOp {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn take_pool(&mut self) -> Option<Lease<Pool>> {
        self.pool.take()
    }

    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
