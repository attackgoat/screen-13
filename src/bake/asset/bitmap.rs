use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct Bitmap {
    force_opaque: Option<bool>,
    src: PathBuf,
}

impl Bitmap {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }

    pub fn force_opaque(&self) -> bool {
        self.force_opaque.unwrap_or_default()
    }
}
