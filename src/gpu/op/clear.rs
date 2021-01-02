use {
    super::Op,
    crate::{
        color::AlphaColor,
        gpu::{
            driver::{CommandPool, Device, Driver, Fence},
            Lease, Pool, Texture2d,
        },
    },
    gfx_hal::{
        command::{ClearValue, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceRange},
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

/// A container of graphics types which allow for effciently setting texture contents.
pub struct ClearOp {
    clear_value: ClearValue,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    fence: Lease<Fence>,
    pool: Option<Lease<Pool>>,
    texture: Texture2d,
}

impl ClearOp {
    #[must_use]
    pub(crate) fn new(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        mut pool: Lease<Pool>,
        texture: &Texture2d,
    ) -> Self {
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(driver, family);

        Self {
            clear_value: AlphaColor::rgba(0, 0, 0, 0).into(),
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(driver),
            fence: pool.fence(
                #[cfg(feature = "debug-names")]
                name,
                driver,
            ),
            pool: Some(pool),
            texture: Texture2d::clone(texture),
        }
    }

    // TODO: Just rename this to color? No need to expose this whole "value" business!
    /// Sets the clear value.
    #[must_use]
    pub fn with_value<C>(&mut self, clear_value: C) -> &mut Self
    where
        C: Into<ClearValue>,
    {
        self.clear_value = clear_value.into();
        self
    }

    /// Submits the given clear for hardware processing.
    pub fn record(&mut self) {
        unsafe {
            self.submit();
        }
    }

    unsafe fn submit(&mut self) {
        trace!("submit");

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

impl Drop for ClearOp {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for ClearOp {
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
