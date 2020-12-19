use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct Material {
    color: PathBuf,
    metal_src: PathBuf,
    normal: PathBuf,
    rough_src: PathBuf,
}

impl Material {
    /// A three or four channel image
    pub fn color(&self) -> &Path {
        self.color.as_path()
    }

    /// A one channel image
    pub fn metal_src(&self) -> &Path {
        self.metal_src.as_path()
    }

    /// A three channel image
    pub fn normal(&self) -> &Path {
        self.normal.as_path()
    }

    /// A one channel image
    pub fn rough_src(&self) -> &Path {
        self.rough_src.as_path()
    }
}
