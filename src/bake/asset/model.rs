use {
    crate::math::{vec3, Vec3},
    ordered_float::OrderedFloat,
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of individual meshes within a `.glb` or `.gltf` 3D model.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Mesh {
    dst_name: Option<String>,
    src_name: String,
}

impl Mesh {
    /// Allows the artist-provided name to be different when referenced by a program.
    pub fn dst_name(&self) -> Option<&str> {
        self.dst_name.as_deref()
    }

    /// The artist-provided name of a mesh within the model.
    pub fn src_name(&self) -> &str {
        &self.src_name
    }
}

/// Holds a description of `.glb` or `.gltf` 3D models.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Model {
    offset: Option<[OrderedFloat<f32>; 3]>,
    scale: Option<[OrderedFloat<f32>; 3]>,
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
        self.offset
            .map(|offset| vec3(offset[0].0, offset[1].0, offset[2].0))
            .unwrap_or(Vec3::ZERO)
    }

    /// Scaling of the model.
    pub fn scale(&self) -> Vec3 {
        self.scale
            .map(|scale| vec3(scale[0].0, scale[1].0, scale[2].0))
            .unwrap_or(Vec3::ONE)
    }

    /// The model file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
