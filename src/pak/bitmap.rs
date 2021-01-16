use {
    crate::math::Extent,
    serde::{Deserialize, Serialize},
};

/// Holds a `Bitmap` in a `.pak` file. For data transport only.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct BitmapBuf {
    fmt: Format,

    #[serde(with = "serde_bytes")]
    pixels: Vec<u8>,

    width: u16,
}

impl BitmapBuf {
    pub(crate) fn new(fmt: Format, width: u16, pixels: Vec<u8>) -> Self {
        Self { fmt, pixels, width }
    }

    /// Gets the dimensions, in pixels, of this `Bitmap`.
    pub fn dims(&self) -> Extent {
        Extent::new(self.width as u32, self.height() as u32)
    }

    /// Gets a description of the numbe of channels contained in this `Bitmap`.
    pub fn format(&self) -> Format {
        self.fmt
    }

    pub(crate) fn height(&self) -> usize {
        let len = self.pixels.len();
        let width = self.width();
        let byte_height = len / width;

        match self.fmt {
            Format::R => byte_height,
            Format::Rg => byte_height / 2,
            Format::Rgb => byte_height / 3,
            Format::Rgba => byte_height >> 2,
        }
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn stride(&self) -> usize {
        self.width() * self.fmt.byte_len()
    }

    pub(crate) fn width(&self) -> usize {
        self.width as _
    }
}

/// Describes the channels of a `Bitmap`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Format {
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

impl Format {
    /// Returns the number of bytes each pixel advances the bitmap stream.
    #[inline]
    pub fn byte_len(self) -> usize {
        match self {
            Self::R => 1,
            Self::Rg => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}
