use {
    super::Op,
    crate::{
        color::AlphaColor,
        gpu::{
            driver::{CommandPool, Fence},
            queue_mut, Lease, Pool, Texture2d,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        command::{ClearValue, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access, Layout, SubresourceRange},
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

/// A container of graphics types which allow for effciently setting texture contents.
pub struct ClearOp<P>
where
    P: 'static + SharedPointerKind,
{
    clear_value: ClearValue,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    fence: Lease<Fence, P>,
    pool: Option<Lease<Pool<P>, P>>,
    texture: Shared<Texture2d, P>,
}

impl<P> ClearOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        texture: &Shared<Texture2d, P>,
    ) -> Self {
        let mut cmd_pool = pool.cmd_pool();

        Self {
            clear_value: AlphaColor::rgba(0, 0, 0, 0).into(),
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            fence: pool.fence(
                #[cfg(feature = "debug-names")]
                name,
            ),
            pool: Some(pool),
            texture: Shared::clone(texture),
        }
    }

    /// Sets the clear value.
    #[must_use]
    pub fn with<C>(&mut self, color: C) -> &mut Self
    where
        C: Into<AlphaColor>,
    {
        self.clear_value = color.into().into();
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

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Clear the image
        self.texture.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            Access::TRANSFER_WRITE,
        );
        self.cmd_buf.clear_image(
            self.texture.as_ref(),
            Layout::TransferDstOptimal,
            self.clear_value,
            once(SubresourceRange {
                aspects: Aspects::COLOR,
                ..Default::default()
            }),
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        queue_mut().submit(once(&self.cmd_buf), empty(), empty(), Some(&mut self.fence));
    }
}

impl<P> Drop for ClearOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait();
        }
    }
}

impl<P> Op<P> for ClearOp<P>
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
