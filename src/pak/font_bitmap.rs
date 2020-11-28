use {
    super::Bitmap,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct FontBitmap {
    def: Vec<u8>,
    pages: Vec<Bitmap>,
}

impl FontBitmap {
    pub(crate) fn new(def: Vec<u8>, pages: Vec<Bitmap>) -> Self {
        Self { def, pages }
    }
}
