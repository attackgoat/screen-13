use {
    super::{wait_for_fence, Op},
    crate::{
        gpu::{
            driver::{CommandPool, Driver, Fence, PhysicalDevice},
            pool::Lease,
            PoolRef, TextureRef,
        },
        math::Extent,
    },
    gfx_hal::{
        command::{CommandBuffer, CommandBufferFlags, ImageCopy, Level},
        format::Aspects,
        image::{Access, Layout, Offset, SubresourceLayers},
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
    dims: Extent,
    driver: Driver,
    dst: TextureRef<D>,
    fence: Lease<Fence>,
    src: TextureRef<S>,
}

impl<S, D> CopyOp<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(pool: &PoolRef, src: &TextureRef<S>, dst: &TextureRef<D>) -> Self {
        let mut pool_ref = pool.borrow_mut();
        let family = pool_ref.driver().borrow_mut().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);

        Self {
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            dims: src.borrow().dims(),
            driver: Driver::clone(pool_ref.driver()),
            dst: TextureRef::clone(dst),
            fence: pool_ref.fence(),
            src: TextureRef::clone(src),
        }
    }

    pub fn with_dims(mut self, dims: Extent) -> Self {
        self.dims = dims;
        self
    }

    pub fn with_dst_layout(mut self, access: Access, layout: Layout) -> Self {
        unsafe {
            self.dst.borrow_mut().set_layout(
                &mut self.cmd_buf,
                layout,
                PipelineStage::BOTTOM_OF_PIPE,
                access,
            );
        }
        self
    }

    pub fn record(mut self) -> impl Op {
        unsafe {
            self.submit();
        };

        CopyResult { op: self }
    }

    unsafe fn submit(&mut self) {
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
                src_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                src_offset: Offset::ZERO,
                dst_subresource: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                dst_offset: Offset::ZERO,
                extent: self.dims.as_extent(1),
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        self.driver.borrow_mut().get_queue_mut(QUEUE_TYPE).submit(
            Submission {
                command_buffers: once(&self.cmd_buf),
                wait_semaphores: empty(),
                signal_semaphores: empty::<&<_Backend as Backend>::Semaphore>(),
            },
            Some(self.fence.as_ref()),
        );
    }
}

#[derive(Debug)]
struct CopyResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    op: CopyOp<S, D>, // TODO: Dump the extras!
}

impl<S, D> Drop for CopyResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<S, D> Op for CopyResult<S, D>
where
    S: AsRef<<_Backend as Backend>::Image>,
    D: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&*self.op.driver.borrow(), &self.op.fence);
        }
    }
}
