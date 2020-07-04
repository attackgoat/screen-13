use {
    super::DataRef,
    crate::math::Extent,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Bitmap {
    has_alpha: bool,
    pixels: DataRef<Vec<u8>>,
    width: u16,
}

impl Bitmap {
    pub fn new(has_alpha: bool, width: u16, pixels: Vec<u8>) -> Self {
        Self {
            has_alpha,
            pixels: DataRef::Data(pixels),
            width,
        }
    }

    pub(crate) fn new_ref(has_alpha: bool, width: u16, pos: u32, len: u32) -> Self {
        Self {
            has_alpha,
            pixels: DataRef::Ref((pos, len)),
            width,
        }
    }

    pub(crate) fn as_ref(&self) -> (u64, usize) {
        self.pixels.as_ref()
    }

    pub fn dims(&self) -> Extent {
        Extent::new(self.width as u32, self.height() as u32)
    }

    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }

    pub fn height(&self) -> usize {
        let len = self.pixels.as_data().len();
        let width = self.width();
        let byte_height = len / width;

        if self.has_alpha {
            byte_height >> 2
        } else {
            byte_height / 3
        }
    }

    pub fn pixels(&self) -> &[u8] {
        match self.pixels {
            DataRef::Data(ref pixels) => pixels,
            _ => unreachable!(),
        }
    }

    pub fn width(&self) -> usize {
        self.width as _
    }
}
