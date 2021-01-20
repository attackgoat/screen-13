use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

const DEFAULT_SCALE: f32 = 32.0;

#[derive(Clone, Deserialize)]
pub struct Font {
    collection_index: Option<u32>,
    scale: Option<f32>,
    src: PathBuf,
}

impl Font {
    /// The index of the font to use if parsing a font collection.
    ///
    /// The default is `0`.
    pub fn collection_index(&self) -> u32 {
        self.collection_index.unwrap_or_default()
    }

    /// The scale in px the font geometry is optimized for.
    ///
    /// Fonts rendered at the scale defined here will be the most optimal in terms of looks and
    /// performance. Glyphs rendered smaller than this scale will look the same but perform slightly
    /// worse, while glyphs rendered larger than this will look worse but perform slightly better.
    ///
    /// The default is `32`
    pub fn scale(&self) -> f32 {
        self.scale.unwrap_or(DEFAULT_SCALE)
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
