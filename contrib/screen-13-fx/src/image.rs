use {anyhow::Context, screen_13::prelude_all::*};

#[derive(Debug)]
pub struct ImageLoader<P>
where
    P: SharedPointerKind,
{
    cache: HashPool<P>,
    _decode_r_rg: Shared<ComputePipeline<P>, P>,
    decode_rgb_rgba: Shared<ComputePipeline<P>, P>,
    pub device: Shared<Device<P>, P>,
}

impl<P> ImageLoader<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            cache: HashPool::new(device),
            _decode_r_rg: Shared::new(ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_BITMAP_R_RG_COMP),
            )?),
            decode_rgb_rgba: Shared::new(ComputePipeline::create(
                device,
                ComputePipelineInfo::new(crate::res::shader::COMPUTE_DECODE_BITMAP_RGB_RGBA_COMP),
            )?),
            device: Shared::clone(device),
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
                    extent: uvec3(bitmap.width(), bitmap.height(), 1),
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
        bitmap: &BitmapBuf,
        is_srgb: bool,
    ) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        info!(
            "Decoding {}x{} {:?} bitmap ({} K)",
            bitmap.width(),
            bitmap.height(),
            bitmap.format(),
            bitmap.pixels().len() / 1024
        );

        let mut render_graph = RenderGraph::new();
        let image = render_graph.bind_node(self.create_image(bitmap, is_srgb, false)?);

        // Fill the image from the temporary buffer
        match bitmap.format() {
            BitmapFormat::R => {
                // This format requires a conversion
                info!("Converting R to RG");
                todo!()
            }
            BitmapFormat::Rgb => {
                // This format requires a conversion
                //info!("Converting RGB to RGBA");

                let (bitmap_width, bitmap_height) = bitmap.extent();
                let bitmap_stride = bitmap.stride();

                //trace!("{bitmap_width}x{bitmap_height} Stride={bitmap_stride}");

                assert_eq!(
                    bitmap_height as usize * bitmap_stride,
                    bitmap.pixels().len()
                );

                let pixel_buf_stride = align_up_u32(bitmap_stride as u32, 12);
                let pixel_buf_len = (pixel_buf_stride * bitmap_height) as u64;

                //trace!("pixel_buf_len={pixel_buf_len} pixel_buf_stride={pixel_buf_stride}");

                // Lease a temporary buffer from the cache pool
                let mut pixel_buf = self.cache.lease(BufferInfo {
                    size: pixel_buf_len,
                    usage: vk::BufferUsageFlags::STORAGE_BUFFER,
                    can_map: true,
                })?;

                {
                    let pixel_buf = pixel_buf.get_mut().unwrap();
                    let pixel_buf =
                        &mut Buffer::mapped_slice_mut(pixel_buf)[0..pixel_buf_len as usize];
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

                let pixel_buf = render_graph.bind_node(pixel_buf);

                // We create a temporary storage image because SRGB support isn't wide enough to
                // have SRGB storage images directly
                let temp_image = render_graph.bind_node(self.create_image(bitmap, false, true)?);

                // Copy host-local data in the buffer to the temporary buffer on the GPU and then
                // use a compute shader to decode it before copying it over the output image

                let dispatch_x = (bitmap_width >> 2) - 1 + (bitmap_width % 3); // HACK: -1 FOR NOW but do fix
                let dispatch_y = bitmap_height;
                render_graph
                    .record_pass("Decode RGB image")
                    .bind_pipeline(&self.decode_rgb_rgba)
                    .read_descriptor(0, pixel_buf)
                    .write_descriptor(1, temp_image)
                    .push_constants(pixel_buf_stride >> 2)
                    .dispatch(dispatch_x, dispatch_y, 1)
                    .submit_pass()
                    .copy_image(temp_image, image);
            }
            BitmapFormat::Rg | BitmapFormat::Rgba => {
                // Lease a temporary buffer from the pool
                let mut pixel_buf = self.cache.lease(BufferInfo {
                    size: bitmap.pixels().len() as _,
                    usage: vk::BufferUsageFlags::TRANSFER_SRC,
                    can_map: true,
                })?;

                {
                    // Fill the temporary buffer with the bitmap pixels
                    let pixel_buf = pixel_buf.get_mut().unwrap();
                    let pixel_buf =
                        &mut Buffer::mapped_slice_mut(pixel_buf)[0..bitmap.pixels().len()];
                    pixel_buf.copy_from_slice(bitmap.pixels());
                }

                let pixel_buf = render_graph.bind_node(pixel_buf);
                render_graph.copy_buffer_to_image(pixel_buf, image);
            }
        }

        let image = render_graph.unbind_node(image);

        render_graph.resolve().submit(&mut self.cache)?;

        Ok(image)
    }

    pub fn decode_linear(&mut self, bitmap: &BitmapBuf) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        self.decode_bitmap(bitmap, false)
    }

    pub fn decode_srgb(&mut self, bitmap: &BitmapBuf) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        self.decode_bitmap(bitmap, true)
    }
}
