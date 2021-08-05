use {
    super::Canonicalize,
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

impl Canonicalize for Blob {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
