use serde::{Deserialize, Serialize};

/// Holds a `Bitmap` in a `.pak` file. For data transport only.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BitmapBuf {
    color: BitmapColor,
    fmt: BitmapFormat,

    #[serde(with = "serde_bytes")]
    pixels: Vec<u8>,

    width: u32,
}

impl BitmapBuf {
    /// Pixel data must be tightly packed (no additional stride)
    pub fn new(
        color: BitmapColor,
        fmt: BitmapFormat,
        width: u32,
        pixels: impl Into<Vec<u8>>,
    ) -> Self {
        let pixels = pixels.into();

        Self {
            color,
            fmt,
            pixels,
            width,
        }
    }

    pub fn color(&self) -> BitmapColor {
        self.color
    }

    /// Gets the dimensions, in pixels, of this `Bitmap`.
    pub fn extent(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    // TODO: Maybe better naming.. Channels?
    /// Gets a description of the number of channels contained in this `Bitmap`.
    pub fn format(&self) -> BitmapFormat {
        self.fmt
    }

    pub fn height(&self) -> u32 {
        let len = self.pixels.len() as u32;
        let byte_height = len / self.width;

        match self.fmt {
            BitmapFormat::R => byte_height,
            BitmapFormat::Rg => byte_height / 2,
            BitmapFormat::Rgb => byte_height / 3,
            BitmapFormat::Rgba => byte_height >> 2,
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> &[u8] {
        let offset = y as usize * self.stride() + x as usize * self.fmt.byte_len();
        &self.pixels[offset..offset + self.fmt.byte_len()]
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_as_format(&self, dst_fmt: BitmapFormat) -> impl Iterator<Item = u8> + '_ {
        let stride = self.fmt.byte_len().min(dst_fmt.byte_len());
        self.pixels
            .chunks(self.fmt.byte_len())
            .flat_map(move |src| {
                let mut dst = [0; 4];
                dst[0..stride].copy_from_slice(&src[0..stride]);
                dst.into_iter()
            })
    }

    /// Bytes per row of pixels (there is no padding)
    pub fn stride(&self) -> usize {
        self.width as usize * self.fmt.byte_len()
    }

    pub fn width(&self) -> u32 {
        self.width
    }
}

/// Describes the channels of a `Bitmap`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum BitmapFormat {
    /// Red channel only.
    #[serde(rename = "r")]
    R,

    /// Red and green channels.
    #[serde(rename = "rg")]
    Rg,

    /// Red, green and blue channels.
    #[serde(rename = "rgb")]
    Rgb,

    /// Red, green, blue and alpha channels.
    #[serde(rename = "rgba")]
    Rgba,
}

impl BitmapFormat {
    /// Returns the number of bytes each pixel advances the bitmap stream.
    #[inline]
    pub const fn byte_len(self) -> usize {
        match self {
            Self::R => 1,
            Self::Rg => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

/// Describes the color space of a `Bitmap`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum BitmapColor {
    #[serde(rename = "linear")]
    Linear,

    #[serde(rename = "srgb")]
    Srgb,
}
