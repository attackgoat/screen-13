use {
    super::{wait_for_fence, Op},
    crate::gpu::{
        driver::{CommandPool, Driver, Fence, PhysicalDevice},
        pool::{Lease, Pool},
        Data, TextureRef,
    },
    gfx_hal::{
        buffer::Access as BufferAccess,
        command::{BufferImageCopy, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access as ImageAccess, Layout, Offset, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    image::{save_buffer, ColorType},
    std::{
        io::Result as IoResult,
        iter::{empty, once},
        path::{Path, PathBuf},
        u8,
    },
};

const DEFAULT_QUALITY: u8 = (0.9f32 * u8::MAX as f32) as u8;
const QUEUE_TYPE: QueueType = QueueType::Graphics;

pub struct Encode<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    op: Option<EncodeOp<I>>,
    path: PathBuf,
}

impl<I> Encode<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub fn flush(&mut self) -> IoResult<()> {
        // We only do this once
        if let Some(op) = self.op.take() {
            self.wait();
            let dims = op.texture.borrow().dims();
            let len = EncodeOp::byte_len(&op.texture);
            let buf = unsafe {
                // Safe because it is waited on above
                op.buf.map_range(0..len as _)
            };

            // Encode the 32bpp RGBA source data into a JPEG
            save_buffer(&self.path, &buf, dims.x, dims.y, ColorType::Rgba8).unwrap();
        }

        Ok(())
    }
}

impl<I> Drop for Encode<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        // If you don't manually call flush errors will be ignored
        self.flush().unwrap_or_default();
    }
}

impl<I> Op for Encode<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn wait(&self) {
        if let Some(op) = &self.op {
            unsafe {
                wait_for_fence(&*op.driver.borrow(), &op.fence);
            }
        }
    }
}

pub struct EncodeOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    buf: Lease<Data>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    fence: Lease<Fence>,
    quality: u8,
    texture: TextureRef<I>,
}

impl<I> EncodeOp<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &mut Pool,
        texture: TextureRef<I>,
    ) -> Self {
        let len = Self::byte_len(&texture);
        let buf = pool.data(
            #[cfg(debug_assertions)]
            name,
            len as _,
        );

        let family = pool.driver().borrow().get_queue_family(QUEUE_TYPE);
        let mut cmd_pool = pool.cmd_pool(family);

        Self {
            buf,
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(pool.driver()),
            fence: pool.fence(),
            quality: DEFAULT_QUALITY,
            texture,
        }
    }

    fn byte_len(texture: &TextureRef<I>) -> usize {
        let dims = texture.borrow().dims();
        (dims.x * dims.y * 4) as _
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality;
        self
    }

    pub fn record<P: AsRef<Path>>(mut self, path: P) -> Encode<I> {
        unsafe {
            self.submit();
        };

        Encode {
            op: Some(self),
            path: path.as_ref().to_owned(),
        }
    }

    unsafe fn submit(&mut self) {
        let len = Self::byte_len(&self.texture);
        let buf = self.buf.as_mut();
        let mut texture = self.texture.borrow_mut();
        let dims = texture.dims();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy the image to our temporary buffer (all on the GPU)
        texture.set_layout(
            &mut self.cmd_buf,
            Layout::TransferSrcOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_READ,
        );
        self.cmd_buf.copy_image_to_buffer(
            &texture.as_ref(),
            Layout::TransferSrcOptimal,
            buf.as_ref(),
            &[BufferImageCopy {
                buffer_offset: 0,
                buffer_width: dims.x,
                buffer_height: dims.y,
                image_layers: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                image_offset: Offset::ZERO,
                image_extent: dims.as_extent(1),
            }],
        );
        buf.pipeline_barrier_gpu(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_WRITE,
        );

        // Step 2: Copy our GPU buffer down to the CPU
        buf.copy_gpu_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_WRITE,
            0..len as _,
        );
        buf.pipeline_barrier_cpu(
            &mut self.cmd_buf,
            PipelineStage::HOST,
            BufferAccess::HOST_READ,
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
