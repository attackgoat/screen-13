use {
    super::{copy_buffer_binding_to_image, copy_image_binding},
    anyhow::Context,
    screen_13::prelude_all::*,
};

#[derive(Debug)]
pub struct ImageLoader<P>
where
    P: SharedPointerKind,
{
    decode_bitmap_r_rg: ComputePipeline<P>,
    decode_bitmap_rgb_rgba: ComputePipeline<P>,
    device: Shared<Device<P>, P>,
    pool: HashPool<P>,
}

impl<P> ImageLoader<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            decode_bitmap_r_rg: ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_BITMAP_R_RG_COMP),
            )?,
            decode_bitmap_rgb_rgba: ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_BITMAP_RGB_RGBA_COMP),
            )?,
            device: Shared::clone(device),
            pool: HashPool::new(device),
        })
    }

    fn create_image(
        &self,
        bitmap: &BitmapBuf,
        is_srgb: bool,
        is_temporary: bool,
    ) -> anyhow::Result<ImageBinding<P>> {
        Ok(ImageBinding::new(
            Image::create(
                &self.device,
                ImageInfo {
                    ty: ImageType::Texture2D,
                    usage: if is_temporary {
                        vk::ImageUsageFlags::STORAGE
                            | vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC
                    } else {
                        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST
                    },
                    flags: vk::ImageCreateFlags::MUTABLE_FORMAT,
                    fmt: match bitmap.format() {
                        BitmapFormat::R | BitmapFormat::Rg => {
                            if is_temporary {
                                vk::Format::R8G8_UINT
                            } else if is_srgb {
                                panic!("Unsupported format: R8G8_SRGB");
                            } else {
                                vk::Format::R8G8_UNORM
                            }
                        }
                        BitmapFormat::Rgb | BitmapFormat::Rgba => {
                            if is_temporary {
                                vk::Format::R8G8B8A8_UINT
                            } else if is_srgb {
                                vk::Format::R8G8B8A8_SRGB
                            } else {
                                vk::Format::R8G8B8A8_UNORM
                            }
                        }
                    },
                    extent: uvec3(bitmap.width, bitmap.height(), 1),
                    tiling: vk::ImageTiling::OPTIMAL,
                    mip_level_count: 1,
                    array_elements: 1,
                    sample_count: SampleCount::X1,
                },
            )
            .context("Unable to create new image")?,
        ))
    }

    pub fn decode_bitmap(
        &mut self,
        bitmap: BitmapBuf,
        is_srgb: bool,
    ) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + 'static,
    {
        info!(
            "Decoding {}x{} {:?} bitmap ({} K)",
            bitmap.width,
            bitmap.height(),
            bitmap.format(),
            bitmap.pixels().len() / 1024
        );

        let cmd_buf = self.pool.lease(self.device.queue.family)?;
        let mut image_binding = self.create_image(&bitmap, is_srgb, false)?;

        // Fill the image from the temporary buffer
        match bitmap.format() {
            BitmapFormat::R => {
                // This format requires a conversion
                info!("Converting R to RG");
                todo!()
            }
            BitmapFormat::Rgb => {
                // This format requires a conversion
                info!("Converting RGB to RGBA");

                let bitmap_width = bitmap.width;
                let bitmap_height = bitmap.height();
                let bitmap_stride = bitmap.stride();

                //trace!("{bitmap_width}x{bitmap_height} Stride={bitmap_stride}");

                assert_eq!(
                    bitmap_height as usize * bitmap_stride,
                    bitmap.pixels().len()
                );

                let pixel_buf_stride = align_up_u32(bitmap_stride as u32, 12);
                let pixel_buf_len = (pixel_buf_stride * bitmap_height) as u64;

                //trace!("pixel_buf_len={pixel_buf_len} pixel_buf_stride={pixel_buf_stride}");

                // Lease a temporary buffer from the pool
                let mut pixel_buf_binding = self.pool.lease(BufferInfo {
                    size: pixel_buf_len,
                    usage: vk::BufferUsageFlags::STORAGE_BUFFER,
                    can_map: true,
                })?;

                {
                    let pixel_buf =
                        &mut Buffer::mapped_slice_mut(pixel_buf_binding.get_mut().unwrap())
                            [0..pixel_buf_len as usize];
                    let pixels = bitmap.pixels();

                    // Fill the temporary buffer with the bitmap pixels - it has a different stride
                    // from the pak data
                    for y in 0..bitmap_height as usize {
                        let src_offset = y * bitmap_stride;
                        let src = &pixels[src_offset..src_offset + bitmap_stride];

                        let dst_offset = y * pixel_buf_stride as usize;
                        let dst = &mut pixel_buf[dst_offset..dst_offset + bitmap_stride];

                        dst.copy_from_slice(src);
                    }
                }

                // We create a temporary storage image because SRGB support isn't wide enough to
                // have SRGB storage images directly
                let mut temp_image_binding = self.create_image(&bitmap, false, true)?;

                // Copy host-local data in the buffer to the temporary buffer on the GPU and then
                // use a compute shader to decode it before copying it over the output image

                let cmd_chain = Self::dispatch_compute_pipeline(
                    cmd_buf,
                    &mut self.pool,
                    &self.decode_bitmap_rgb_rgba,
                    pixel_buf_binding,
                    &mut temp_image_binding,
                    (bitmap_width >> 2) + (bitmap_width % 3),
                    bitmap_height,
                    Some(pixel_buf_stride >> 2),
                )?;
                copy_image_binding(cmd_chain, &mut temp_image_binding, &mut image_binding)
            }
            BitmapFormat::Rg | BitmapFormat::Rgba => {
                // Lease a temporary buffer from the pool
                let mut pixel_buf_binding = self.pool.lease(BufferInfo {
                    size: bitmap.pixels().len() as _,
                    usage: vk::BufferUsageFlags::TRANSFER_SRC,
                    can_map: true,
                })?;

                // Fill the temporary buffer with the bitmap pixels
                Buffer::mapped_slice_mut(pixel_buf_binding.get_mut().unwrap())
                    [0..bitmap.pixels().len()]
                    .copy_from_slice(bitmap.pixels());

                // These formats match the output image so just copy the bytes straight over
                copy_buffer_binding_to_image(cmd_buf, &mut pixel_buf_binding, &mut image_binding)
            }
        }
        .submit()?;

        Ok(image_binding)
    }

    pub fn decode_linear(&mut self, bitmap: BitmapBuf) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + 'static,
    {
        self.decode_bitmap(bitmap, false)
    }

    pub fn decode_srgb(&mut self, bitmap: BitmapBuf) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + 'static,
    {
        self.decode_bitmap(bitmap, true)
    }

    fn dispatch_compute_pipeline<Ch, Cb>(
        cmd_chain: Ch,
        pool: &mut HashPool<P>,
        pipeline: &ComputePipeline<P>,
        mut pixel_buf_binding: Lease<BufferBinding<P>, P>,
        image_binding: &mut ImageBinding<P>,
        group_count_x: u32,
        group_count_y: u32,
        push_constants: Option<u32>,
    ) -> Result<CommandChain<Cb, P>, anyhow::Error>
    where
        Ch: Into<CommandChain<Cb, P>>,
        Cb: AsRef<CommandBuffer<P>>,
        P: 'static,
    {
        use std::slice::from_ref;

        // Raw vulkan pipeline handles
        let descriptor_set_layout = &pipeline.descriptor_info.layouts[&0];
        let pipeline_layout = pipeline.layout;
        let pipeline = **pipeline;

        // Raw vulkan buffer handle
        let (pixel_buf, previous_pixel_buf_access, _) =
            pixel_buf_binding.access_inner(AccessType::ComputeShaderReadOther);
        let pixel_buf = **pixel_buf;

        // Raw vulkan image/view handles
        let (image, previous_image_access, _) =
            image_binding.access_inner(AccessType::ComputeShaderWrite);
        let image_view_info = image.info.into();
        let image_view = Image::view_ref(image, image_view_info)?;
        let image = **image;

        // Allocate a single descriptor set from the pool (This set is exclusive for this dispatch)
        let descriptor_pool = pool.lease(DescriptorPoolInfo::new(1).pool_sizes(vec![
            DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
            },
            DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 1,
            },
        ]))?;
        let descriptor_set_ref =
            DescriptorPool::allocate_descriptor_set(&descriptor_pool, descriptor_set_layout)?;
        let descriptor_set = *descriptor_set_ref;

        // Write the descriptors for our pixel buffer source and image destination
        unsafe {
            descriptor_pool.device.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(from_ref(&vk::DescriptorBufferInfo {
                            buffer: pixel_buf,
                            offset: 0,
                            range: vk::WHOLE_SIZE,
                        }))
                        .build(),
                    vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(1)
                        .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                        .image_info(from_ref(&vk::DescriptorImageInfo {
                            sampler: vk::Sampler::null(),
                            image_view,
                            image_layout: vk::ImageLayout::GENERAL,
                        }))
                        .build(),
                ],
                &[],
            )
        }

        Ok(cmd_chain
            .into()
            .push_shared_ref(descriptor_pool)
            .push_shared_ref(descriptor_set_ref)
            .push_shared_ref(pixel_buf_binding)
            .push_shared_ref(image_binding.shared_ref())
            .push_execute(move |device, cmd_buf| unsafe {
                CommandBuffer::buffer_barrier(
                    cmd_buf,
                    previous_pixel_buf_access,
                    AccessType::ComputeShaderReadOther,
                    pixel_buf,
                    None,
                );
                CommandBuffer::image_barrier(
                    cmd_buf,
                    previous_image_access,
                    AccessType::ComputeShaderWrite,
                    image,
                    None,
                );

                device.cmd_bind_pipeline(**cmd_buf, vk::PipelineBindPoint::COMPUTE, pipeline);
                device.cmd_bind_descriptor_sets(
                    **cmd_buf,
                    vk::PipelineBindPoint::COMPUTE,
                    pipeline_layout,
                    0,
                    from_ref(&descriptor_set),
                    &[],
                );

                if let Some(data) = push_constants {
                    device.cmd_push_constants(
                        **cmd_buf,
                        pipeline_layout,
                        vk::ShaderStageFlags::COMPUTE,
                        0,
                        as_u8_slice(&data),
                    );
                }

                device.cmd_dispatch(**cmd_buf, group_count_x, group_count_y, 1);
            }))
    }
}
