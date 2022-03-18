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
use {super::Writer, log::info, parking_lot::Mutex, std::sync::Arc};

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
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
    ) -> anyhow::Result<BitmapId> {
        self.bake_from_source(writer, project_dir, None as Option<&'static str>)
    }

    #[cfg(feature = "bake")]
    /// Reads and processes image source files into an existing `.pak` file buffer.
    pub fn bake_from_source(
        &mut self,
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> anyhow::Result<BitmapId> {
        // Early-out if we have already baked this bitmap
        let asset = self.clone().into();
        if let Some(id) = writer.lock().ctx.get(&asset) {
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

        let bitmap = self
            .as_bitmap_buf()
            .context("Unable to create bitmap buf")?;

        let mut writer = writer.lock();
        if let Some(id) = writer.ctx.get(&asset) {
            return Ok(id.as_bitmap().unwrap());
        }

        Ok(writer.push_bitmap(bitmap, key))
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
        //let started = std::time::Instant::now();

        /*
            If this section ends up being very slow, it is usually because image was built in debug
            mode. You can use this for a regular build:

            [profile.dev.package.image]
            opt-level = 3

            But for a build.rs script you will need something a bit more invasive:

            [profile.dev.build-override]
            opt-level = 3 # Makes image 10x faster
            codegen-units = 1 # Makes image 2x faster (stacks with the above!)

            Obviously this will trade build time for runtime performance. PR this if you have better
            methods of handling this!!
        */

        let mut image = open(&path)
            .with_context(|| format!("Unable to open image file: {}", path.as_ref().display()))?;

        //let elapsed = std::time::Instant::now() - started;
        //info!("Image open took {} ms for {}x{}", elapsed.as_millis(), image.width(), image.height());

        if let Some(resize) = resize {
            let (width, height) = if image.width() > image.height() {
                (resize, resize * image.height() / image.width())
            } else {
                (resize * image.width() / image.height(), resize)
            };
            let filter_ty = if image.width() == 1 && image.height() == 1 {
                FilterType::Nearest
            } else {
                FilterType::CatmullRom
            };

            image = image.resize_to_fill(width, height, filter_ty);
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
