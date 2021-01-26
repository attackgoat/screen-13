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
    pub fn new<S>(src: S) -> Self
    where
        S: AsRef<Path>,
    {
        Self {
            format: None,
            src: src.as_ref().to_path_buf(),
        }
    }

    pub fn with_format(mut self, fmt: BitmapFormat) -> Self {
        self.format = Some(fmt);
        self
    }

    pub fn with_format_is(mut self, fmt: Option<BitmapFormat>) -> Self {
        self.format = fmt;
        self
    }

    pub fn format(&self) -> BitmapFormat {
        self.format.unwrap_or(BitmapFormat::Rgba)
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
