use {
    crate::math::Extent,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Bitmap {
    fmt: Format,

    #[serde(with = "serde_bytes")]
    pixels: Vec<u8>,

    width: u16,
}

impl Bitmap {
    pub(crate) fn new(fmt: Format, width: u16, pixels: Vec<u8>) -> Self {
        Self { fmt, pixels, width }
    }

    pub fn dims(&self) -> Extent {
        Extent::new(self.width as u32, self.height() as u32)
    }

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
        let bytes = match self.fmt {
            Format::R => 1,
            Format::Rg => 2,
            Format::Rgb => 3,
            Format::Rgba => 4,
        };

        self.width() * bytes
    }

    pub(crate) fn width(&self) -> usize {
        self.width as _
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Format {
    #[serde(rename = "r")]
    R,

    #[serde(rename = "rg")]
    Rg,

    #[serde(rename = "rgb")]
    Rgb,

    #[serde(rename = "rgba")]
    Rgba,
}
