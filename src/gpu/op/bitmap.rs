use {
    super::{wait_for_fence, Op},
    crate::{
        gpu::{
            driver::{
                bind_compute_descriptor_set, change_channel_type, CommandPool, ComputePipeline,
                Device, Driver, Fence, Image2d, PhysicalDevice,
            },
            pool::{Compute, ComputeMode, Lease},
            Data, PoolRef, TextureRef,
        },
        math::Extent,
        pak::Bitmap as PakBitmap,
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{CommandBuffer, CommandBufferFlags, Level},
        device::Device as _,
        format::{ChannelType, Format},
        image::{Access as ImageAccess, Layout, Tiling, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage},
        queue::{CommandQueue as _, QueueType, Submission},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        iter::{empty, once},
        ops::Deref,
        u64,
    },
};

const QUEUE_TYPE: QueueType = QueueType::Compute;

/// Holds a decoded 2D 4-channel image.
#[derive(Debug)]
pub struct Bitmap {
    op: BitmapOp, // TODO: Dump the extras!
}

impl Deref for Bitmap {
    type Target = TextureRef<Image2d>;

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
        let device = self.op.driver.borrow();

        unsafe {
            wait_for_fence(&device, &self.op.fence);
        }
    }
}

#[derive(Debug)]
pub struct BitmapOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    compute: Lease<Compute>,
    dims: Extent,
    dispatch_x: u32,
    driver: Driver,
    fence: Lease<Fence>,
    pixel_buf: Lease<Data>,
    pixel_buf_len: u64,
    pixel_buf_stride: u32,
    texture: Lease<TextureRef<Image2d>>,
}

impl BitmapOp {
    /// # Safety
    /// None
    pub unsafe fn new(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        bitmap: &PakBitmap,
        format: Format,
    ) -> Self {
        let dims = bitmap.dims();
        let width: usize = dims.x as _;
        let height: usize = dims.y as _;

        // Figure out what kind of bitmap we're decoding
        let (mode, dispatch_x, bitmap_stride, pixel_buf_stride) = if bitmap.has_alpha() {
            //(ComputeMode::DecodeBgra32, 0, 0, 0)
            todo!()
        } else {
            let dispatch_x = (dims.x >> 2) + (dims.x % 3);
            let bitmap_stride = width * 3;
            let mod_12 = bitmap_stride % 12;
            let pixel_buf_stride = bitmap_stride + 12 - mod_12;
            (
                ComputeMode::DecodeBgr24,
                dispatch_x,
                bitmap_stride,
                pixel_buf_stride,
            )
        };

        // Lease some data from the pool
        let pool = PoolRef::clone(pool);
        let mut pool_ref = pool.borrow_mut();
        let pixel_buf_len = width * pixel_buf_stride;
        let pixel_buf = pool_ref.data_usage(
            #[cfg(debug_assertions)]
            name,
            pixel_buf_len as _,
            BufferUsage::STORAGE,
        );

        {
            // Fill the cpu-side buffer with our pixel data
            // The data is packed pixel-to-pixel in the pak data because it needs to be small for the wire,
            // but to speed up the actual image decode process we need to feed the GPU pixel data where each
            // row allows for additional buffer space on it, which we use stride to track. At this point we
            // must convert from pak-format to gpu-format by copying in each row.
            let src = bitmap.pixels();
            let mut dst = pixel_buf.map_range_mut(0..pixel_buf_len as _);
            for y in 0..height {
                let src_offset = y * bitmap_stride;
                let dst_offset = y * pixel_buf_stride;
                dst[dst_offset..dst_offset + bitmap_stride]
                    .copy_from_slice(&src[src_offset..src_offset + bitmap_stride]);
            }
        }

        // Lease a texture to hold the decoded bitmap
        let texture = pool_ref.texture(
            #[cfg(debug_assertions)]
            name,
            bitmap.dims(),
            Tiling::Optimal, // TODO: Use is_supported and is_compatible to figure this out
            format,
            Layout::Undefined,
            ImageUsage::STORAGE
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            1,
            1,
            1,
        );

        let compute = pool_ref.compute(
            #[cfg(debug_assertions)]
            name,
            mode,
        );

        // Allocate the command buffer
        let family = Device::queue_family(&pool_ref.driver().borrow(), QUEUE_TYPE);
        let mut cmd_pool = pool_ref.cmd_pool(family);

        Self {
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            compute,
            dims: bitmap.dims(),
            dispatch_x,
            driver: Driver::clone(pool_ref.driver()),
            fence: pool_ref.fence(),
            pixel_buf,
            pixel_buf_len: pixel_buf_len as _,
            pixel_buf_stride: pixel_buf_stride as _,
            texture,
        }
    }

    /// # Safety
    ///
    /// None
    pub fn record(mut self) -> Bitmap {
        unsafe {
            self.write_descriptors();
            self.submit();
        };

        Bitmap { op: self }
    }

    unsafe fn submit(&mut self) {
        let mut device = self.driver.borrow_mut();
        let mut texture = self.texture.borrow_mut();
        let pipeline = self.compute.pipeline();
        let layout = ComputePipeline::layout(&pipeline);
        let desc_set = self.compute.desc_set(0);

        // Begin
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        // Step 1: Copy the local cpu memory buffer into the gpu-local buffer
        self.pixel_buf.copy_cpu(
            &mut self.cmd_buf,
            PipelineStage::COMPUTE_SHADER,
            BufferAccess::SHADER_READ,
            self.pixel_buf_len,
        );

        // Step 2: Use a compute shader to remap the memory layout of the device-local buffer
        texture.set_layout(
            &mut self.cmd_buf,
            Layout::General,
            PipelineStage::COMPUTE_SHADER,
            ImageAccess::SHADER_WRITE,
        );
        self.cmd_buf.bind_compute_pipeline(pipeline);
        self.cmd_buf
            .push_compute_constants(layout, 0, &[self.pixel_buf_stride >> 2]);
        bind_compute_descriptor_set(&mut self.cmd_buf, layout, desc_set);
        self.cmd_buf.dispatch([self.dispatch_x, self.dims.y, 1]);

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

    unsafe fn write_descriptors(&mut self) {
        let texture = self.texture.borrow();
        let texture_view = texture
            .as_default_2d_view_format(change_channel_type(texture.format(), ChannelType::Uint));
        self.driver.borrow().write_descriptor_sets(
            vec![
                DescriptorSetWrite {
                    set: self.compute.desc_set(0),
                    binding: 0,
                    array_offset: 0,
                    descriptors: once(Descriptor::Buffer(
                        &*self.pixel_buf.as_ref(),
                        SubRange {
                            offset: 0,
                            size: Some(self.pixel_buf_len),
                        },
                    )),
                },
                DescriptorSetWrite {
                    set: self.compute.desc_set(0),
                    binding: 1,
                    array_offset: 0,
                    descriptors: once(Descriptor::Image(texture_view.as_ref(), Layout::General)), // TODO ????? Shouldn't this not be general?
                },
            ]
            .drain(..),
        );
    }
}
