use {
    crate::pak::BitmapFormat,
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct Bitmap {
    format: Option<BitmapFormat>,
    src: PathBuf,
}

impl Bitmap {
    pub fn format(&self) -> BitmapFormat {
        self.format.unwrap_or(BitmapFormat::Rgba)
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
