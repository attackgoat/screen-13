use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of `.fnt` bitmapped fonts.
#[derive(Clone, Deserialize)]
pub struct BitmapFont {
    src: PathBuf,
}

impl BitmapFont {
    /// The bitmapped font file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
