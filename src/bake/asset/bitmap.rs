use {
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Bitmap {
    force_opaque: bool,
    src: PathBuf,
}

impl Bitmap {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }

    pub fn force_opaque(&self) -> bool {
        self.force_opaque
    }
}
