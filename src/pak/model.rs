use {
    crate::math::{Mat4, Sphere},
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        ops::Range,
    },
};

#[derive(Deserialize, PartialEq, Serialize)]
pub struct Batch {
    indices: Range<u32>,
    mode: TriangleMode,
}

impl Batch {
    pub fn new(indices: Range<u32>, mode: TriangleMode) -> Self {
        Self { indices, mode }
    }

    pub fn indices(&self) -> Range<u32> {
        self.indices.clone()
    }

    pub fn mode(&self) -> TriangleMode {
        self.mode
    }
}

#[derive(Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    batches: Vec<Batch>,
    bounds: Sphere,
    name: Option<String>,
    skin_inv_binds: Option<HashMap<String, Mat4>>,
    transform: Option<Mat4>,
}

impl Mesh {
    pub fn new<N: Into<Option<String>>>(
        batches: Vec<Batch>,
        bounds: Sphere,
        name: N,
        transform: Option<Mat4>,
        skin_inv_binds: Option<HashMap<String, Mat4>>,
    ) -> Self {
        Self {
            batches,
            bounds,
            name: name.into(),
            skin_inv_binds,
            transform,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Deserialize, PartialEq, Serialize)]
pub struct Model {
    indices: Vec<u8>,
    meshes: Vec<Mesh>,
    vertices: Vec<u8>,
}

impl Model {
    pub(crate) fn new(mut meshes: Vec<Mesh>, indices: Vec<u8>, vertices: Vec<u8>) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);

        meshes.sort_unstable_by(|lhs, rhs| lhs.name().cmp(&rhs.name()));

        Self {
            indices,
            meshes,
            vertices,
        }
    }

    pub(crate) fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub(crate) fn take_meshes(self) -> Vec<Mesh> {
        self.meshes
    }

    pub(crate) fn vertices(&self) -> &[u8] {
        &self.vertices
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TriangleMode {
    Fan,
    List,
    Strip,
}
