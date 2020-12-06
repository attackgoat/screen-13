use {
    super::Bitmap,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct BitmapFont {
    def: String,
    pages: Vec<Bitmap>,
}

impl BitmapFont {
    pub(crate) fn new(def: String, pages: Vec<Bitmap>) -> Self {
        Self { def, pages }
    }

    pub fn def(&self) -> &str {
        self.def.as_str()
    }

    pub fn pages(&self) -> impl Iterator<Item = &Bitmap> {
        self.pages.iter()
    }
}
