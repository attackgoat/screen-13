use {
    super::Mesh,
    crate::math::Vec3,
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Model {
    offset: Option<Vec3>,
    scale: Option<Vec3>,
    src: PathBuf,
    #[serde(rename = "mesh")]
    meshes: Vec<Mesh>,
}

impl Model {
    pub fn new<P: AsRef<Path>>(src: P, offset: Vec3, scale: Vec3) -> Self {
        Self {
            meshes: vec![],
            offset: Some(offset),
            scale: Some(scale),
            src: src.as_ref().to_owned(),
        }
    }

    pub fn meshes(&self) -> &[Mesh] {
        &self.meshes
    }

    pub fn offset(&self) -> Vec3 {
        self.offset.unwrap_or(Vec3::zero())
    }

    pub fn scale(&self) -> Vec3 {
        self.scale.unwrap_or(Vec3::one())
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
