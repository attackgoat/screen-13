use {
    super::{file_key, BitmapBuf, BitmapId, Canonicalize},
    crate::pak::{BitmapColor, BitmapFormat},
    anyhow::Context,
    image::{buffer::ConvertBuffer, imageops::FilterType, open, DynamicImage, RgbaImage},
    serde::Deserialize,
    std::{
        io::{Error, ErrorKind},
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use {super::Writer, log::info};

/// Holds a description of `.jpeg` and other regular images.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Bitmap {
    color: Option<BitmapColor>,
    format: Option<BitmapFormat>,
    resize: Option<u32>,
    src: PathBuf,
}

impl Bitmap {
    /// Constructs a new Bitmap with the given image file source.
    pub fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            color: None,
            format: None,
            resize: None,
            src: src.as_ref().to_path_buf(),
        }
    }

    pub fn with_color(mut self, color: BitmapColor) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_format(mut self, format: BitmapFormat) -> Self {
        self.format = Some(format);
        self
    }

    #[cfg(feature = "bake")]
    /// Reads and processes image source files into an existing `.pak` file buffer.
    pub fn bake(
        &mut self,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
    ) -> anyhow::Result<BitmapId> {
        self.bake_from_source(writer, project_dir, None as Option<&'static str>)
    }

    #[cfg(feature = "bake")]
    /// Reads and processes image source files into an existing `.pak` file buffer.
    pub fn bake_from_source(
        &mut self,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> anyhow::Result<BitmapId> {
        // Early-out if we have already baked this bitmap
        if let Some(id) = writer.ctx.get(&self.clone().into()) {
            return Ok(id.as_bitmap().unwrap());
        }

        let key = src.as_ref().map(|src| file_key(&project_dir, src));
        if let Some(key) = &key {
            // This bitmap will be accessible using this key
            info!("Baking bitmap: {}", key);
        } else {
            // This bitmap will only be accessible using the id
            info!(
                "Baking bitmap: {} (inline)",
                file_key(&project_dir, self.src())
            );
        }

        // If format was not specified we guess (it is read as it is from disk; this
        // is just format represented in the .pak file and what you can retrieve it as)
        if self.format.is_none() {
            if let Some(src) = &src {
                self.format = match open(src).context("Unable to open bitmap file")? {
                    DynamicImage::ImageLuma8(_) => Some(BitmapFormat::R),
                    DynamicImage::ImageRgb8(_) => Some(BitmapFormat::Rgb),
                    DynamicImage::ImageRgba8(img) => {
                        if img.pixels().all(|pixel| pixel[3] == u8::MAX) {
                            // The source image has alpha but we're going to discard it
                            Some(BitmapFormat::Rgb)
                        } else {
                            Some(BitmapFormat::Rgba)
                        }
                    }
                    _ => None,
                };
            }
        }

        let id = writer.push_bitmap(
            self.as_bitmap_buf()
                .context("Unable to create bitmap buf")?,
            key,
        );

        Ok(id)
    }

    pub fn as_bitmap_buf(&self) -> anyhow::Result<BitmapBuf> {
        let (width, pixels) = Self::read_pixels(self.src(), self.format(), self.resize)
            .context("Unable to read pixels")?;

        Ok(BitmapBuf::new(self.color(), self.format(), width, pixels))
    }

    pub fn color(&self) -> BitmapColor {
        self.color.unwrap_or(BitmapColor::Srgb)
    }

    /// Specific pixel channels used.
    pub fn format(&self) -> BitmapFormat {
        self.format.unwrap_or(BitmapFormat::Rgba)
    }

    /// Reads raw pixel data from an image source file and returns them in the given format.
    pub fn read_pixels(
        path: impl AsRef<Path>,
        fmt: BitmapFormat,
        resize: Option<u32>,
    ) -> anyhow::Result<(u32, Vec<u8>)> {
        let mut image = open(path).context("Unable to open image file")?;

        if let Some(resize) = resize {
            let (width, height) = if image.width() > image.height() {
                (resize, resize * image.height() / image.width())
            } else {
                (resize * image.width() / image.height(), resize)
            };
            image = image.resize_to_fill(width, height, FilterType::CatmullRom);
        }

        let image = match image {
            DynamicImage::ImageLuma8(image) => image.convert(),
            DynamicImage::ImageLumaA8(image) => image.convert(),
            DynamicImage::ImageRgb8(image) => image.convert(),
            DynamicImage::ImageRgba8(image) => image,
            DynamicImage::ImageLuma16(image) => image.convert(),
            DynamicImage::ImageLumaA16(image) => image.convert(),
            DynamicImage::ImageRgb16(image) => image.convert(),
            DynamicImage::ImageRgba16(image) => image.convert(),
            DynamicImage::ImageRgb32F(image) => image.convert(),
            DynamicImage::ImageRgba32F(image) => image.convert(),
            _ => unimplemented!(),
        };
        let width = image.width();
        let data = match fmt {
            BitmapFormat::R => Self::pixels_r(&image),
            BitmapFormat::Rg => Self::pixels_rg(&image),
            BitmapFormat::Rgb => Self::pixels_rgb(&image),
            BitmapFormat::Rgba => Self::pixels_rgba(&image),
        };

        Ok((width, data))
    }

    fn pixels_r(image: &RgbaImage) -> Vec<u8> {
        let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, image.height() - y - 1);
                buf.push(pixel[0]);
            }
        }

        buf
    }

    fn pixels_rg(image: &RgbaImage) -> Vec<u8> {
        let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 2);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, image.height() - y - 1);
                buf.push(pixel[0]);
                buf.push(pixel[1]);
            }
        }

        buf
    }

    fn pixels_rgb(image: &RgbaImage) -> Vec<u8> {
        let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, image.height() - y - 1);
                buf.push(pixel[0]);
                buf.push(pixel[1]);
                buf.push(pixel[2]);
            }
        }

        buf
    }

    fn pixels_rgba(image: &RgbaImage) -> Vec<u8> {
        let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, image.height() - y - 1);
                buf.push(pixel[0]);
                buf.push(pixel[1]);
                buf.push(pixel[2]);
                buf.push(pixel[3]);
            }
        }

        buf
    }

    /// The image file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Bitmap {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
