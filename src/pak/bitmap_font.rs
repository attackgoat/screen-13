use {
    super::BitmapBuf,
    serde::{Deserialize, Serialize},
};

/// Holds a `BitmapFont` in a `.pak` file. For data transport only.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct BitmapFont {
    def: String,
    pages: Vec<BitmapBuf>,
}

impl BitmapFont {
    pub(crate) fn new(def: String, pages: Vec<BitmapBuf>) -> Self {
        Self { def, pages }
    }

    // TODO: We could pre-pack this instead of raw text!
    /// Gets the main `.fnt` file in original text form
    pub fn def(&self) -> &str {
        self.def.as_str()
    }

    /// Gets an iterator of `Bitmap` pages within this `BitmapFont`.
    pub fn pages(&self) -> impl Iterator<Item = &BitmapBuf> {
        self.pages.iter()
    }
}
