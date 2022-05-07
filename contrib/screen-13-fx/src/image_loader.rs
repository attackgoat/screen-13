use {
    super::BitmapFont,
    anyhow::Context,
    archery::{SharedPointer, SharedPointerKind},
    bmfont::BMFont,
    inline_spirv::include_spirv,
    screen_13::prelude_all::*,
};

fn align_up_u32(val: u32, atom: u32) -> u32 {
    (val + atom - 1) & !(atom - 1)
}

/// Describes the channels and pixel stride of an image format
#[derive(Clone, Copy, Debug)]
pub enum ImageFormat {
    R8,
    R8G8,
    R8G8B8,
    R8G8B8A8,
}

impl ImageFormat {
    fn stride(self) -> usize {
        match self {
            Self::R8 => 1,
            Self::R8G8 => 2,
            Self::R8G8B8 => 3,
            Self::R8G8B8A8 => 4,
        }
    }
}

#[derive(Debug)]
pub struct ImageLoader<P>
where
    P: SharedPointerKind,
{
    cache: HashPool<P>,
    _decode_r_rg: SharedPointer<ComputePipeline<P>, P>,
    decode_rgb_rgba: SharedPointer<ComputePipeline<P>, P>,
    pub device: SharedPointer<Device<P>, P>,
}

impl<P> ImageLoader<P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn new(device: &SharedPointer<Device<P>, P>) -> Result<Self, DriverError> {
        Ok(Self {
            cache: HashPool::new(device),
            _decode_r_rg: SharedPointer::new(ComputePipeline::create(
                device,
                include_spirv!("res/shader/compute/decode_bitmap_r_rg.comp", comp).as_slice(),
            )?),
            decode_rgb_rgba: SharedPointer::new(ComputePipeline::create(
                device,
                include_spirv!("res/shader/compute/decode_bitmap_rgb_rgba.comp", comp).as_slice(),
            )?),
            device: SharedPointer::clone(device),
        })
    }

    fn create_image(
        &self,
        format: ImageFormat,
        width: u32,
        height: u32,
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
                        vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC
                    },
                    flags: vk::ImageCreateFlags::empty(),
                    fmt: match format {
                        ImageFormat::R8 | ImageFormat::R8G8 => {
                            if is_temporary {
                                vk::Format::R8G8_UINT
                            } else if is_srgb {
                                panic!("Unsupported format: R8G8_SRGB");
                            } else {
                                vk::Format::R8G8_UNORM
                            }
                        }
                        ImageFormat::R8G8B8 | ImageFormat::R8G8B8A8 => {
                            if is_temporary {
                                vk::Format::R8G8B8A8_UINT
                            } else if is_srgb {
                                vk::Format::R8G8B8A8_SRGB
                            } else {
                                vk::Format::R8G8B8A8_UNORM
                            }
                        }
                    },
                    width,
                    height,
                    depth: 1,
                    linear_tiling: false,
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
        pixels: &[u8],
        format: ImageFormat,
        width: u32,
        height: u32,
        is_srgb: bool,
    ) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        info!(
            "decoding {}x{} {:?} bitmap ({} K)",
            width,
            height,
            format,
            pixels.len() / 1024
        );

        debug_assert!(
            pixels.len() >= format.stride() * (width * height) as usize,
            "insufficient data"
        );

        #[cfg(debug_assertions)]
        if pixels.len() >= format.stride() * (width * height) as usize {
            warn!("unused data");
        }

        let mut render_graph = RenderGraph::new();
        let image =
            render_graph.bind_node(self.create_image(format, width, height, is_srgb, false)?);

        // Fill the image from the temporary buffer
        match format {
            ImageFormat::R8 => {
                // This format requires a conversion
                info!("Converting R to RG");
                todo!()
            }
            ImageFormat::R8G8B8 => {
                // This format requires a conversion
                //info!("Converting RGB to RGBA");

                let stride = width * format.stride() as u32;

                //trace!("{bitmap_width}x{bitmap_height} Stride={bitmap_stride}");

                let pixel_buf_stride = align_up_u32(stride as u32, 12);
                let pixel_buf_len = (pixel_buf_stride * height) as u64;

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

                    // Fill the temporary buffer with the bitmap pixels - it has a different stride
                    for y in 0..height {
                        let src_offset = y * stride;
                        let src = &pixels[src_offset as usize..(src_offset + stride) as usize];

                        let dst_offset = y * pixel_buf_stride;
                        let dst =
                            &mut pixel_buf[dst_offset as usize..(dst_offset + stride) as usize];

                        dst.copy_from_slice(src);
                    }
                }

                let pixel_buf = render_graph.bind_node(pixel_buf);

                // We create a temporary storage image because SRGB support isn't wide enough to
                // have SRGB storage images directly
                let temp_image =
                    render_graph.bind_node(self.create_image(format, width, height, false, true)?);

                // Copy host-local data in the buffer to the temporary buffer on the GPU and then
                // use a compute shader to decode it before copying it over the output image

                let dispatch_x = (width >> 2) - 1 + (width % 3); // HACK: -1 FOR NOW but do fix
                let dispatch_y = height;
                render_graph
                    .begin_pass("Decode RGB image")
                    .bind_pipeline(&self.decode_rgb_rgba)
                    .read_descriptor(0, pixel_buf)
                    .write_descriptor(1, temp_image)
                    .record_compute(move |compute| {
                        compute
                            .push_constants(&(pixel_buf_stride >> 2).to_ne_bytes())
                            .dispatch(dispatch_x, dispatch_y, 1);
                    })
                    .submit_pass()
                    .copy_image(temp_image, image);
            }
            ImageFormat::R8G8 | ImageFormat::R8G8B8A8 => {
                // Lease a temporary buffer from the pool
                let mut pixel_buf = self.cache.lease(BufferInfo::new_mappable(
                    pixels.len() as _,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                ))?;

                {
                    // Fill the temporary buffer with the bitmap pixels
                    let pixel_buf = pixel_buf.get_mut().unwrap();
                    let pixel_buf = &mut Buffer::mapped_slice_mut(pixel_buf)[0..pixels.len()];
                    pixel_buf.copy_from_slice(pixels);
                }

                let pixel_buf = render_graph.bind_node(pixel_buf);
                render_graph.copy_buffer_to_image(pixel_buf, image);
            }
        }

        let image = render_graph.unbind_node(image);

        render_graph.resolve().submit(&mut self.cache)?;

        Ok(image)
    }

    pub fn decode_linear(
        &mut self,
        pixels: &[u8],
        format: ImageFormat,
        width: u32,
        height: u32,
    ) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        self.decode_bitmap(pixels, format, width, height, false)
    }

    pub fn decode_srgb(
        &mut self,
        pixels: &[u8],
        format: ImageFormat,
        width: u32,
        height: u32,
    ) -> anyhow::Result<ImageBinding<P>>
    where
        P: SharedPointerKind + Send + 'static,
    {
        self.decode_bitmap(pixels, format, width, height, true)
    }

    pub fn load_bitmap_font<'a>(
        &mut self,
        font: BMFont,
        pages: impl IntoIterator<Item = (&'a [u8], u32, u32)>,
    ) -> anyhow::Result<BitmapFont<P>> {
        let pages = pages
            .into_iter()
            .map(|(pixels, width, height)| {
                self.decode_linear(pixels, ImageFormat::R8G8B8, width, height)
            })
            .collect::<Result<Vec<_>, _>>()?;

        BitmapFont::new(&self.device, font, pages)
    }
}
