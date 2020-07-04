use {
    super::{wait_for_fence, Op},
    crate::{
        color::AlphaColor,
        gpu::{
            driver::{CommandPool, Driver, Fence, PhysicalDevice},
            pool::{Lease, Pool},
            TextureRef,
        },
    },
    gfx_hal::{
        command::{ClearValue, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceRange},
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
struct Clear<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    op: ClearOp<I>, // TODO: Dump the extras?
}

impl<I> Drop for Clear<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<I> Op for Clear<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        unsafe {
            wait_for_fence(&*self.op.driver.borrow(), &self.op.fence);
        }
    }
}

#[derive(Debug)]
pub struct ClearOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    clear_value: ClearValue,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    fence: Lease<Fence>,
    texture: TextureRef<I>,
}

impl<I> ClearOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(pool: &mut Pool, texture: &TextureRef<I>) -> Self {
        let family = pool.driver().borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool.cmd_pool(family);
        Self {
            clear_value: AlphaColor::rgba(0, 0, 0, 0).into(),
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(pool.driver()),
            fence: pool.fence(),
            texture: TextureRef::clone(texture),
        }
    }

    pub fn with_clear_value<C>(mut self, clear_value: C) -> Self
    where
        C: Into<ClearValue>,
    {
        self.clear_value = clear_value.into();
        self
    }

    pub fn record(mut self) -> impl Op {
        unsafe {
            self.submit();
        };

        Clear { op: self }
    }

    unsafe fn submit(&mut self) {
        let mut texture = self.texture.borrow_mut();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Clear the image
        texture.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.clear_image(
            texture.as_ref(),
            Layout::TransferDstOptimal,
            self.clear_value,
            &[SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            }],
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
