use {
    super::BitmapBuf,
    serde::{Deserialize, Serialize},
};

/// Holds a `BitmapFont` in a `.pak` file. For data transport only.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct BitmapFont {
    def: String,
    page: BitmapBuf,
}

impl BitmapFont {
    pub(crate) fn new(def: String, page: BitmapBuf) -> Self {
        Self { def, page }
    }

    // TODO: We could pre-pack this instead of raw text!
    /// Gets the main `.fnt` file in original text form
    pub fn def(&self) -> &str {
        self.def.as_str()
    }

    /// Gets the single `BitmapBuf` page within this `BitmapFont`.
    ///
    /// If a given `BMFont` specifies multiple pages, they will be stacked in the Y+ direction
    /// and so form one tall "page".
    pub fn page(&self) -> &BitmapBuf {
        &self.page
    }
}
