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
    std::iter::{empty, once},
};

pub struct CopyOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: Texture2d,
    dst_offset: Extent,
    fence: Lease<Fence>,
    region: Extent,
    src: Texture2d,
    src_offset: Extent,
}

impl CopyOp {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        pool: &mut Pool,
        src: &Texture2d,
        dst: &Texture2d,
    ) -> Self {
        let (cmd_buf, cmd_pool, fence) = {
            let family = Device::queue_family(&driver.borrow());
            let mut cmd_pool = pool.cmd_pool(driver, family);
            let fence = pool.fence(
                #[cfg(debug_assertions)]
                name,
                driver,
            );

            let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };

            (cmd_buf, cmd_pool, fence)
        };

        Self {
            cmd_buf,
            cmd_pool,
            driver: Driver::clone(driver),
            dst: Texture2d::clone(dst),
            dst_offset: Extent::ZERO,
            fence,
            region: src.borrow().dims(),
            src: Texture2d::clone(src),
            src_offset: Extent::ZERO,
        }
    }

    /// Specifies an identically-sized area of the source and destination to copy, and the position on the
    /// destination where the data will go.
    pub fn with_region(&mut self, src_region: Area, dst: Extent) -> &mut Self {
        self.dst_offset = dst;
        self.region = src_region.dims;
        self.src_offset = src_region.pos;
        self
    }

    pub fn record(mut self) -> impl Op {
        unsafe {
            self.submit();
        };

        CopyOpSubmission {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            driver: self.driver,
            dst: self.dst,
            fence: self.fence,
            src: self.src,
        }
    }

    unsafe fn submit(&mut self) {
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

pub struct CopyOpSubmission {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: Texture2d,
    fence: Lease<Fence>,
    src: Texture2d,
}

impl Drop for CopyOpSubmission {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for CopyOpSubmission {
    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
