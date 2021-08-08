use {
    super::Canonicalize,
    crate::pak::BitmapFormat,
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of `.jpeg` and other regular images.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Bitmap {
    format: Option<BitmapFormat>,
    src: PathBuf,
}

impl Bitmap {
    /// Constructs a new Bitmap with the given image file source.
    pub fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            format: None,
            src: src.as_ref().to_path_buf(),
        }
    }

    pub(crate) fn with_format(mut self, fmt: BitmapFormat) -> Self {
        self.format = Some(fmt);
        self
    }

    pub(crate) fn with_format_is(mut self, fmt: Option<BitmapFormat>) -> Self {
        self.format = fmt;
        self
    }

    /// Specific pixel channels used.
    pub fn format(&self) -> BitmapFormat {
        self.format.unwrap_or(BitmapFormat::Rgba)
    }

    /// The image file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Bitmap {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
