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
    // pub(crate) fn new<P: AsRef<Path>>(src: P, offset: Vec3, scale: Vec3) -> Self {
    //     Self {
    //         meshes: Some(vec![]),
    //         offset: Some(offset),
    //         scale: Some(scale),
    //         src: src.as_ref().to_owned(),
    //     }
    // }

    // TODO: Write an iterator or something this is temporary!
    /// The list of meshes within a model.
    pub fn meshes(&self) -> &Option<Vec<Mesh>> {
        &self.meshes
    }

    /// Translation of the model origin.
    pub fn offset(&self) -> Vec3 {
        self.offset.unwrap_or_else(|| Vec3::ZERO)
    }

    /// Scaling of the model.
    pub fn scale(&self) -> Vec3 {
        self.scale.unwrap_or_else(|| Vec3::ONE)
    }

    /// The model file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
