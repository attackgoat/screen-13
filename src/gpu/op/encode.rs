use {
    super::Op,
    crate::gpu::{
        driver::{CommandPool, Device, Driver, Fence, PhysicalDevice},
        pool::{Lease, Pool},
        Data, Texture2d,
    },
    gfx_hal::{
        command::{BufferImageCopy, CommandBuffer, CommandBufferFlags, Level},
        format::Aspects,
        image::{Access as ImageAccess, Layout, Offset, SubresourceLayers},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::{CommandQueue as _, Submission},
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

pub struct Encode {
    op: Option<EncodeOp>,
    path: PathBuf,
}

impl Encode {
    pub fn flush(&mut self) -> IoResult<()> {
        // We only do this once
        if let Some(mut op) = self.op.take() {
            self.wait();
            let dims = op.texture.borrow().dims();
            let len = EncodeOp::byte_len(&op.texture);
            let buf = op.buf.map_range(0..len as _).unwrap(); // TODO: Error handling!

            // Encode the 32bpp RGBA source data into a JPEG
            save_buffer(&self.path, &buf, dims.x, dims.y, ColorType::Rgba8).unwrap();
        }

        Ok(())
    }
}

impl Drop for Encode {
    fn drop(&mut self) {
        // If you don't manually call flush errors will be ignored
        self.flush().unwrap_or_default();
    }
}

impl Op for Encode {
    fn wait(&self) {
        if let Some(op) = &self.op {
            Fence::wait(&op.fence);
        }
    }
}

pub struct EncodeOp {
    buf: Lease<Data>,
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    driver: Driver,
    fence: Lease<Fence>,
    quality: u8,
    texture: Texture2d,
}

impl EncodeOp {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        pool: &mut Pool,
        texture: Texture2d,
    ) -> Self {
        let len = Self::byte_len(&texture);
        let buf = pool.data(
            #[cfg(debug_assertions)]
            name,
            driver,
            len as _,
        );

        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(driver, family);

        Self {
            buf,
            cmd_buf: unsafe { cmd_pool.allocate_one(Level::Primary) },
            cmd_pool,
            driver: Driver::clone(driver),
            fence: pool.fence(
                #[cfg(debug_assertions)]
                name,
                driver,
            ),
            quality: DEFAULT_QUALITY,
            texture,
        }
    }

    pub fn with_quality(&mut self, quality: u8) -> &mut Self {
        self.quality = quality;
        self
    }

    fn byte_len(texture: &Texture2d) -> usize {
        let dims = texture.borrow().dims();
        (dims.x * dims.y * 4) as _
    }

    pub fn record<P: AsRef<Path>>(mut self, path: P) -> Encode {
        unsafe {
            self.submit();
        };

        Encode {
            op: Some(self),
            path: path.as_ref().to_owned(),
        }
    }

    unsafe fn submit(&mut self) {
        let mut device = self.driver.borrow_mut();
        let len = Self::byte_len(&self.texture);
        let buf = &mut *self.buf;
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
                image_extent: dims.as_extent_depth(1),
            }],
        );

        // Step 2: Copy our GPU buffer down to the CPU
        buf.read_range(&mut self.cmd_buf, 0..len as _);

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
