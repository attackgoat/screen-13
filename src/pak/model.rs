use {
    super::DataRef,
    crate::math::{Mat4, Sphere},
    serde::{Deserialize, Serialize},
    std::ops::Range,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    bounds: Sphere,
    indices: Range<u32>,
    name: Option<String>,
    transform: Option<Mat4>,
    tri_mode: TriangleMode,
}

impl Mesh {
    pub(crate) fn new<N: Into<Option<String>>>(
        bounds: Sphere,
        indices: Range<u32>,
        name: N,
        transform: Option<Mat4>,
        tri_mode: TriangleMode,
    ) -> Self {
        Self {
            bounds,
            indices,
            name: name.into(),
            transform,
            tri_mode,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    indices: Vec<u8>,
    meshes: Vec<Mesh>,
    vertices: Vec<u8>,
}

impl Model {
    pub(crate) fn new(meshes: Vec<Mesh>, indices: Vec<u8>, vertices: Vec<u8>) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);

        Self {
            indices,
            meshes,
            vertices,
        }
    }

    pub(crate) fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub(crate) fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter()
    }

    pub(crate) fn vertices(&self) -> &[u8] {
        &self.vertices
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TriangleMode {
    Fan,
    List,
    Strip,
}
