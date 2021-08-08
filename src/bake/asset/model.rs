use {
    super::Canonicalize,
    crate::math::{vec3, Vec3},
    ordered_float::OrderedFloat,
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of individual meshes within a `.glb` or `.gltf` 3D model.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Mesh {
    name: String,
    rename: Option<String>,
}

impl Mesh {
    /// The artist-provided name of a mesh within the model.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Allows the artist-provided name to be different when referenced by a program.
    pub fn rename(&self) -> Option<&str> {
        let rename = self.rename.as_deref();
        if let Some("") = rename {
            None
        } else {
            rename
        }
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
    pub(crate) fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
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

impl Canonicalize for Model {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
