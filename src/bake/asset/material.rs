use {
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Material {
    albedo: PathBuf,
    metal: PathBuf,
    normal: PathBuf,
}

impl Material {
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        albedo: P1,
        normal: P2,
        metal: P3,
    ) -> Self {
        Self {
            albedo: albedo.as_ref().to_path_buf(),
            metal: metal.as_ref().to_path_buf(),
            normal: normal.as_ref().to_path_buf(),
        }
    }

    pub fn albedo(&self) -> &Path {
        self.albedo.as_path()
    }

    pub fn metal(&self) -> &Path {
        self.metal.as_path()
    }

    pub fn normal(&self) -> &Path {
        self.normal.as_path()
    }
}
