use {
    super::Op,
    crate::{
        gpu::{
            align_up,
            data::Mapping,
            driver::{
                bind_compute_descriptor_set, change_channel_type, CommandPool, ComputePipeline,
                Device, Driver, Fence, PhysicalDevice,
            },
            pool::{Compute, ComputeMode, Lease, Pool},
            Data, Texture2d,
        },
        math::Coord,
        pak::{Bitmap as PakBitmap, BitmapFormat},
    },
    gfx_hal::{
        buffer::{Access as BufferAccess, SubRange, Usage as BufferUsage},
        command::{BufferImageCopy, CommandBuffer, CommandBufferFlags, Level},
        device::Device as _,
        format::{Aspects, ChannelType, Format, ImageFeature, SurfaceType},
        image::{Access as ImageAccess, Layout, SubresourceLayers, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{Descriptor, DescriptorSetWrite, PipelineStage},
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
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    conv_fmt: Option<Lease<Compute>>,
    fence: Lease<Fence>,
    pixel_buf: Lease<Data>,
    texture: Lease<Texture2d>,
}

impl Deref for Bitmap {
    type Target = Texture2d;

    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        self.wait();
    }
}

impl Op for Bitmap {
    fn wait(&self) {
        Fence::wait(&self.fence);
    }
}

pub struct BitmapOp {
    cmd_buf: <_Backend as Backend>::CommandBuffer,
    cmd_pool: Lease<CommandPool>,
    conv_fmt: Option<ComputeDispatch>,
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
        driver: &Driver,
        pool: &mut Pool,
        bitmap: &PakBitmap,
    ) -> Self {
        // Lease a texture to hold the decoded bitmap
        let desired_fmts: &[Format] = match bitmap.format() {
            BitmapFormat::R => &[
                Format::R8Unorm,
                Format::Rg8Unorm,
                Format::Rgb8Unorm,
                Format::Rgba8Unorm,
            ],
            BitmapFormat::Rg => &[Format::Rg8Unorm, Format::Rgb8Unorm, Format::Rgba8Unorm],
            BitmapFormat::Rgb => &[Format::Rgb8Unorm, Format::Rgba8Unorm],
            BitmapFormat::Rgba => &[Format::Rgba8Unorm],
        };
        let texture = pool.texture(
            #[cfg(debug_assertions)]
            name,
            driver,
            bitmap.dims(),
            desired_fmts,
            Layout::Undefined,
            ImageUsage::SAMPLED
                | ImageUsage::STORAGE
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            ImageFeature::SAMPLED | ImageFeature::STORAGE,
            1,
            1,
            1,
        );

        // Figure out what kind of bitmap we're decoding
        let bitmap_stride = bitmap.stride();
        let texture_fmt = texture.borrow().format();
        let conv_fmt = if texture_fmt == desired_fmts[0] {
            // No format conversion: We will use a simple copy-buffer-to-image command
            None
        } else {
            // Format conversion: We will use a compute shader to convert buffer-to-image
            let width = bitmap.dims().x;
            let surface_ty = texture_fmt.base_format().0;
            let (mode, dispatch_count, pixel_buf_stride) = match bitmap.format() {
                // BitmapFormat::R => match surface_ty {
                //     SurfaceType::R8_G8,
                //     SurfaceType::R8_G8_B8,
                //     SurfaceType::B8_G8_R8,
                //     SurfaceType::R8_G8_B8_A8,
                //     SurfaceType::B8_G8_R8_A8,
                //     SurfaceType::A8_B8_G8_R8,
                // }
                // BitmapFormat::Rg => match surface_ty {
                //     SurfaceType::R8_G8_B8,
                //     SurfaceType::B8_G8_R8,
                //     SurfaceType::R8_G8_B8_A8,
                //     SurfaceType::B8_G8_R8_A8,
                //     SurfaceType::A8_B8_G8_R8,
                // }
                BitmapFormat::Rgb => {
                    let dispatch = (width >> 2) + (width % 3);
                    let stride = align_up(bitmap_stride as u32, 12);
                    match surface_ty {
                        SurfaceType::R8_G8_B8_A8 => (ComputeMode::DecodeRgbRgba, dispatch, stride),
                        // SurfaceType::B8_G8_R8_A8 => todo!(),
                        // SurfaceType::A8_B8_G8_R8 => todo!(),
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            };

            let compute = pool.compute(
                #[cfg(debug_assertions)]
                name,
                driver,
                mode,
            );

            Some(ComputeDispatch {
                compute,
                dispatch_count,
                pixel_buf_stride,
            })
        };

        // Lease some data from the pool
        let height = bitmap.height();
        let pixel_buf_len = bitmap_stride * height;
        let mut pixel_buf = pool.data_usage(
            #[cfg(debug_assertions)]
            name,
            driver,
            pixel_buf_len as _,
            if conv_fmt.is_some() {
                BufferUsage::STORAGE
            } else {
                BufferUsage::TRANSFER_SRC
            },
        );

        {
            let src = bitmap.pixels();
            let mut dst = pixel_buf.map_range_mut(0..pixel_buf_len as _).unwrap(); // TODO: Error handling
            let pixel_buf_stride = conv_fmt
                .as_ref()
                .map(|c| c.pixel_buf_stride as _)
                .unwrap_or(bitmap_stride);

            // Fill the cpu-side buffer with our pixel data
            if bitmap_stride == pixel_buf_stride {
                copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), pixel_buf_len);
            } else {
                // At this point we must convert from pak-format to shader-format by copying in each row.
                for y in 0..height {
                    let src_offset = y * bitmap_stride;
                    let dst_offset = y * pixel_buf_stride;
                    dst[dst_offset..dst_offset + bitmap_stride]
                        .copy_from_slice(&src[src_offset..src_offset + bitmap_stride]);
                }

                Mapping::flush(&mut dst).unwrap(); // TODO: Error handling
            }
        }

        // Allocate the command buffer
        let family = Device::queue_family(&driver.borrow());
        let mut cmd_pool = pool.cmd_pool(driver, family);

        Self {
            cmd_buf: cmd_pool.allocate_one(Level::Primary),
            cmd_pool,
            conv_fmt,
            driver: Driver::clone(driver),
            fence: pool.fence(
                #[cfg(debug_assertions)]
                name,
                driver,
            ),
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
            if self.conv_fmt.is_some() {
                self.write_descriptors();
            }

            self.submit_begin();

            if self.conv_fmt.is_some() {
                self.submit_conv();
            } else {
                self.submit_copy();
            }

            self.submit_finish();
        };

        Bitmap {
            cmd_buf: self.cmd_buf,
            cmd_pool: self.cmd_pool,
            conv_fmt: self.conv_fmt.map(|c| c.compute),
            fence: self.fence,
            pixel_buf: self.pixel_buf,
            texture: self.texture,
        }
    }

    unsafe fn submit_begin(&mut self) {
        self.cmd_buf
            .begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
    }

    unsafe fn submit_conv(&mut self) {
        let conv_fmt = self.conv_fmt.as_ref().unwrap();
        let desc_set = conv_fmt.compute.desc_set(0);
        let pipeline = conv_fmt.compute.pipeline();
        let layout = ComputePipeline::layout(&pipeline);
        let mut texture = self.texture.borrow_mut();
        let dims = texture.dims();

        // Step 1: Write the local cpu memory buffer into the gpu-local buffer
        self.pixel_buf.write_range(
            &mut self.cmd_buf,
            PipelineStage::COMPUTE_SHADER,
            BufferAccess::SHADER_READ,
            0..self.pixel_buf_len,
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
            .push_compute_constants(layout, 0, &[conv_fmt.pixel_buf_stride >> 2]);
        bind_compute_descriptor_set(&mut self.cmd_buf, layout, desc_set);
        self.cmd_buf.dispatch([conv_fmt.dispatch_count, dims.y, 1]);
    }

    unsafe fn submit_copy(&mut self) {
        let mut texture = self.texture.borrow_mut();
        let dims = texture.dims();

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
    }

    unsafe fn submit_finish(&mut self) {
        let mut device = self.driver.borrow_mut();

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

    unsafe fn write_descriptors(&mut self) {
        let conv_fmt = self.conv_fmt.as_ref().unwrap();

        let texture = self.texture.borrow();
        let texture_view = texture
            .as_default_view_format(change_channel_type(texture.format(), ChannelType::Uint));
        self.driver.borrow().write_descriptor_sets(vec![
            DescriptorSetWrite {
                set: conv_fmt.compute.desc_set(0),
                binding: 0,
                array_offset: 0,
                descriptors: once(Descriptor::Buffer(
                    self.pixel_buf.as_ref(),
                    SubRange {
                        offset: 0,
                        size: Some(self.pixel_buf_len),
                    },
                )),
            },
            DescriptorSetWrite {
                set: conv_fmt.compute.desc_set(0),
                binding: 1,
                array_offset: 0,
                descriptors: once(Descriptor::Image(texture_view.as_ref(), Layout::General)), // TODO ????? Shouldn't this not be general?
            },
        ]);
    }
}

struct ComputeDispatch {
    compute: Lease<Compute>,
    dispatch_count: u32,
    pixel_buf_stride: u32,
}
