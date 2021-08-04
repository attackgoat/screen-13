use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of any generic file.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Blob {
    src: PathBuf,
}

impl Blob {
    /// The file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
