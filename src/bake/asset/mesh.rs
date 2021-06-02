use serde::Deserialize;

/// Holds a description of individual meshes within a `.glb` or `.gltf` 3D model.
#[derive(Clone, Deserialize)]
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
