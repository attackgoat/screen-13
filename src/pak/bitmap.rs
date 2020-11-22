use {
    super::DataRef,
    crate::math::Extent,
    serde::{Deserialize, Serialize},
    std::ops::Range,
};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Bitmap {
    fmt: Format,
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

    pub fn fmt(&self) -> Format {
        self.fmt
    }

    pub(crate) fn height(&self) -> usize {
        let len = self.pixels.len();
        let width = self.width();
        let byte_height = len / width;

        match self.fmt {
            Format::Rgba => byte_height >> 2,
            Format::Rgb => byte_height / 3,
        }
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn width(&self) -> usize {
        self.width as _
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Format {
    Rgb,
    Rgba,
}
