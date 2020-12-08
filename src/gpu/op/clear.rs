use {
    super::Op,
    crate::{
        color::AlphaColor,
        gpu::{
            driver::{CommandPool, Device, Driver, Fence, PhysicalDevice},
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
    pub fn new(#[cfg(debug_assertions)] name: &str, driver: &Driver, pool: &mut Pool, texture: &TextureRef<I>) -> Self {
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(driver, family);
        Self {
            clear_value: AlphaColor::rgba(0, 0, 0, 0).into(),
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(driver),
            fence: pool.fence(#[cfg(debug_assertions)] name, driver),
            texture: TextureRef::clone(texture),
        }
    }

    pub fn with_clear_value<C>(&mut self, clear_value: C) -> &mut Self
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

        ClearOpSubmission {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            driver: self.driver,
            fence: self.fence,
            texture: self.texture,
        }
    }

    unsafe fn submit(&mut self) {
        let mut device = self.driver.borrow_mut();
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
                ..Default::default()
            }],
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

pub struct ClearOpSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    fence: Lease<Fence>,
    texture: TextureRef<I>,
}

impl<I> Drop for ClearOpSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        self.wait();
    }
}

impl<I> Op for ClearOpSubmission<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}
