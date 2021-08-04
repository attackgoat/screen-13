use {
    super::Op,
    crate::{
        gpu::{
            driver::{CommandPool, Fence},
            pool::{Lease, Pool},
            queue_mut, Data, Texture2d,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        buffer::Access as BufferAccess,
        command::{BufferImageCopy, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access as ImageAccess, Layout, Offset, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::Queue as _,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    image::{save_buffer, ColorType},
    std::{
        any::Any,
        io::Result as IoResult,
        iter::{empty, once},
        path::{Path, PathBuf},
    },
};

const DEFAULT_QUALITY: f32 = 0.9;

// TODO: Quality isn't hooked up!

/// A container of graphics types which allow the recording of encode operations and the saving
/// of renders to disk as regular image files.
pub struct EncodeOp<P>
where
    P: 'static + SharedPointerKind,
{
    buf: Lease<Data, P>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool, P>,
    fence: Lease<Fence, P>,
    pool: Option<Lease<Pool<P>, P>>,
    path: Option<PathBuf>,
    quality: f32,
    texture: Shared<Texture2d, P>,
}

impl<P> EncodeOp<P>
where
    P: SharedPointerKind,
{
    #[must_use]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        mut pool: Lease<Pool<P>, P>,
        texture: &Shared<Texture2d, P>,
    ) -> Self {
        let len = Self::byte_len(texture);
        let buf = pool.data(
            #[cfg(feature = "debug-names")]
            name,
            len as _,
            true,
        );

        let mut cmd_pool = pool.cmd_pool();

        Self {
            buf,
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            fence: pool.fence(
                #[cfg(feature = "debug-names")]
                name,
            ),
            pool: Some(pool),
            path: None,
            quality: DEFAULT_QUALITY,
            texture: Shared::clone(texture),
        }
    }

    // TODO: This is non-functional, hook it up!
    /// Sets the quality to encode with.
    #[must_use]
    pub fn with_quality(&mut self, quality: f32) -> &mut Self {
        self.quality = quality;
        self
    }

    fn byte_len(texture: &Texture2d) -> usize {
        let dims = texture.dims();
        (dims.x * dims.y * 4) as _
    }

    /// Waits for the hardware to finish processing all images, returning an error if something
    /// went wrong.
    ///
    /// _NOTE:_ The program will panic if there is an error while flushing _and_ you have not
    /// manually called the `flush` function. The `flush` function is called automatically when
    /// an `EncodeOp` is dropped.
    pub fn flush(&mut self) -> IoResult<()> {
        // We only do this once
        if let Some(path) = self.path.take() {
            unsafe {
                self.wait();
            }

            let dims = self.texture.dims();
            let len = Self::byte_len(&self.texture);
            let buf = self.buf.map_range(0..len as _).unwrap(); // TODO: Error handling!

            // Encode the 32bpp RGBA source data into a JPEG
            save_buffer(path, &buf, dims.x, dims.y, ColorType::Rgba8).unwrap();
        }

        Ok(())
    }

    /// Submits the given encode for hardware processing.
    pub fn record<A: AsRef<Path>>(&mut self, path: A) {
        self.path = Some(path.as_ref().to_path_buf());

        unsafe {
            self.submit();
        };
    }

    unsafe fn submit(&mut self) {
        trace!("submit");

        let len = Self::byte_len(&self.texture);
        let buf = &mut *self.buf;
        let dims = self.texture.dims();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy the image to our temporary buffer (all on the GPU)
        self.texture.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        self.cmd_buf.copy_image_to_buffer(
            self.texture.as_ref(),
            Layout::TransferSrcOptimal,
            buf.as_ref(),
            once(BufferImageCopy {
                buffer_offset: 0,
                buffer_width: dims.x,
                buffer_height: dims.y,
                image_layers: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                image_offset: Offset::ZERO,
                image_extent: dims.as_extent_depth(1),
            }),
        );

        // Step 2: Copy our GPU buffer down to the CPU
        buf.read_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_WRITE,
            0..len as _,
        );

        // Finish
        self.cmd_buf.finish();

        // Submit
        queue_mut().submit(once(&self.cmd_buf), empty(), empty(), Some(&mut self.fence));
    }
}

impl<P> Drop for EncodeOp<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        // If you don't manually call flush errors will be ignored
        self.flush().unwrap_or_default();
    }
}

impl<P> Op<P> for EncodeOp<P>
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
