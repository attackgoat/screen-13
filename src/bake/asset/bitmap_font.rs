use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct BitmapFont {
    src: PathBuf,
}

impl BitmapFont {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
