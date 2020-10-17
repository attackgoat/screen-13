use {
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct FontBitmap {
    src: PathBuf,
}

impl FontBitmap {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
