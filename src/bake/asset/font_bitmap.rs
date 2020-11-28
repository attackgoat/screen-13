use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct FontBitmap {
    src: PathBuf,
}

impl FontBitmap {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
