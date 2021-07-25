use {
    super::Mesh,
    crate::math::Vec3,
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of `.glb` or `.gltf` 3D models.
#[derive(Clone, Deserialize)]
pub struct Model {
    offset: Option<Vec3>,
    scale: Option<Vec3>,
    src: PathBuf,
    #[serde(rename = "mesh")]
    meshes: Option<Vec<Mesh>>,
}

impl Model {
    pub(crate) fn new<P: AsRef<Path>>(src: P) -> Self {
        Self {
            meshes: None,
            offset: None,
            scale: None,
            src: src.as_ref().to_owned(),
        }
    }

    /// The list of meshes within a model.
    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter().flatten()
    }

    /// Translation of the model origin.
    pub fn offset(&self) -> Vec3 {
        self.offset.unwrap_or(Vec3::ZERO)
    }

    /// Scaling of the model.
    pub fn scale(&self) -> Vec3 {
        self.scale.unwrap_or(Vec3::ONE)
    }

    /// The model file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
