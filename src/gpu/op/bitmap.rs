use {
    super::Op,
    crate::{
        gpu::{
            data::Mapping,
            driver::{CommandPool, Device, Driver, Fence, PhysicalDevice},
            pool::Lease,
            Data, PoolRef, Texture2d,
        },
        math::{Coord, Extent},
        pak::{Bitmap as PakBitmap, BitmapFormat},
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, Usage as BufferUsage},
        command::{BufferImageCopy, CommandBuffer, CommandBufferFlags, Level},
        format::{Aspects, Format},
        image::{Access as ImageAccess, Layout, SubresourceLayers, Tiling, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::PipelineStage,
        queue::{CommandQueue as _, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::{empty, once},
        ops::Deref,
        ptr::copy_nonoverlapping,
        u64,
    },
};

/// Holds a decoded 2D image with 1-4 channels.
pub struct Bitmap {
    op: BitmapOp, // TODO: Dump the extras!
}

impl Deref for Bitmap {
    type Target = Texture2d;

    fn deref(&self) -> &Self::Target {
        &self.op.texture
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for Bitmap {
    fn wait(&self) {
        Fence::wait(&self.op.fence);
    }
}

pub struct BitmapOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    dims: Extent,
    driver: Driver,
    fence: Lease<Fence>,
    pixel_buf: Lease<Data>,
    pixel_buf_len: u64,
    texture: Lease<Texture2d>,
}

impl BitmapOp {
    /// # Safety
    /// None
    pub unsafe fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        bitmap: &PakBitmap,
    ) -> Self {
        // Lease some data from the pool
        let pool = PoolRef::clone(pool);
        let mut pool_ref = pool.borrow_mut();
        let pixel_buf_len = bitmap.stride() * bitmap.height();
        let mut pixel_buf = pool_ref.data_usage(
            #[cfg(debug_assertions)]
            name,
            pixel_buf_len as _,
            BufferUsage::TRANSFER_SRC,
        );

        {
            // Fill the cpu-side buffer with our pixel data
            let src = bitmap.pixels();
            let mut dst = pixel_buf.map_range_mut(0..pixel_buf_len as _).unwrap(); // TODO: Error handling
            copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), pixel_buf_len);

            Mapping::flush(&mut dst).unwrap(); // TODO: Error handling
        }

        let desired_fmts = match bitmap.format() {
            BitmapFormat::R => &[Format::R8Unorm],
            BitmapFormat::Rg => &[Format::Rg8Unorm],
            BitmapFormat::Rgb => &[Format::Rgb8Unorm],
            BitmapFormat::Rgba => &[Format::Rgba8Unorm],
        };

        // Lease a texture to hold the decoded bitmap
        let texture = pool_ref.texture(
            #[cfg(debug_assertions)]
            name,
            bitmap.dims(),
            Tiling::Optimal, // TODO: Use is_supported and is_compatible to figure this out
            desired_fmts,
            Layout::Undefined,
            ImageUsage::STORAGE
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );

        // Allocate the command buffer
        let family = Device::queue_family(&pool_ref.driver().borrow());
        let mut cmd_pool = pool_ref.cmd_pool(family);

        Self {
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            dims: bitmap.dims(),
            driver: Driver::clone(pool_ref.driver()),
            fence: pool_ref.fence(),
            pixel_buf,
            pixel_buf_len: pixel_buf_len as _,
            texture,
        }
    }

    /// # Safety
    ///
    /// None
    pub fn record(mut self) -> Bitmap {
        unsafe {
            self.submit();
        };

        Bitmap { op: self }
    }

    unsafe fn submit(&mut self) {
        let mut device = self.driver.borrow_mut();
        let mut texture = self.texture.borrow_mut();
        let dims = texture.dims();

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Write the local cpu memory buffer into the gpu-local buffer
        self.pixel_buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::TRANSFER,
            BufferAccess::TRANSFER_READ,
            0..self.pixel_buf_len,
        );

        // Step 2: Copy the buffer to the image
        texture.set_layout(
            &mut self.cmd_buf,
            Layout::TransferDstOptimal,
            PipelineStage::TRANSFER,
            ImageAccess::TRANSFER_WRITE,
        );
        self.cmd_buf.copy_buffer_to_image(
            self.pixel_buf.as_ref(),
            texture.as_ref(),
            Layout::TransferDstOptimal,
            &[BufferImageCopy {
                buffer_offset: 0,
                buffer_width: dims.x,
                buffer_height: dims.y,
                image_layers: SubresourceLayers {
                    aspects: Aspects::COLOR,
                    level: 0,
                    layers: 0..1,
                },
                image_offset: Coord::ZERO.into(),
                image_extent: dims.as_extent_depth(1),
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
