use {
    super::{wait_for_fence, Op},
    crate::{
        gpu::{
            driver::{CommandPool, Device, Driver, Fence, PhysicalDevice},
            pool::Lease,
            PoolRef, TextureRef,
        },
        math::{Area, Extent},
    },
    gfx_hal::{
        command::{CommandBuffer as _, CommandBufferFlags, ImageCopy, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

const QUEUE_TYPE: QueueType = QueueType::Graphics;

#[derive(Debug)]
pub struct CopyOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    dst: TextureRef<D>,
    dst_offset: Extent,
    fence: Lease<Fence>,
    region: Extent,
    src: TextureRef<S>,
    src_offset: Extent,
}

impl<S, D> CopyOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(pool: &PoolRef, src: &TextureRef<S>, dst: &TextureRef<D>) -> Self {
        let (cmd_buf, cmd_pool, driver, fence) = {
            let mut pool_ref = pool.borrow_mut();
            let family = Device::queue_family(&pool_ref.driver().borrow(), QUEUE_TYPE);
            let mut cmd_pool = pool_ref.cmd_pool(family);
            let driver = Driver::clone(pool_ref.driver());
            let fence = pool_ref.fence();

            let cmd_buf = unsafe { cmd_pool.allocate_one(Level::Primary) };

            (cmd_buf, cmd_pool, driver, fence)
        };

        Self {
            cmd_buf,
            cmd_pool,
            driver,
            dst: TextureRef::clone(dst),
            dst_offset: Extent::ZERO,
            fence,
            region: src.borrow().dims(),
            src: TextureRef::clone(src),
            src_offset: Extent::ZERO,
        }
    }

    /// Specifies an identically-sized area of the source and destination to copy, and the position on the
    /// destination where the data will go.
    pub fn with_region(mut self, src_region: Area, dst: Extent) -> Self {
        self.dst_offset = dst;
        self.region = src_region.dims;
        self.src_offset = src_region.pos;
        self
    }

    pub fn record(mut self) -> impl Op {
        unsafe {
            self.submit();
        };

        self
    }

    unsafe fn submit(&mut self) {
        let mut device = self.driver.borrow_mut();
        let mut src = self.src.borrow_mut();
        let mut dst = self.dst.borrow_mut();

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
                dst_offset: self.dst_offset.as_offset(0),
                extent: self.region.as_extent(1),
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset: self.src_offset.as_offset(0),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        Device::queue_mut(&mut device, QUEUE_TYPE).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(&self.fence),
        );
    }
}

impl<S, D> Drop for CopyOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<S, D> Op for CopyOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        let device = self.driver.borrow();

        unsafe {
            wait_for_fence(&device, &self.fence);
        }
    }
}
