use {
    super::IndexType,
    crate::math::{Mat4, Sphere},
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        ops::Range,
    },
};

#[derive(Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    batches: Vec<Range<u32>>,
    bounds: Sphere,
    name: Option<String>,
    skin_inv_binds: Option<HashMap<String, Mat4>>,
    transform: Option<Mat4>,
    vertex_base: u64,
}

impl Mesh {
    pub fn new<N: Into<Option<String>>>(
        batches: Vec<Range<u32>>,
        bounds: Sphere,
        name: N,
        transform: Option<Mat4>,
        skin_inv_binds: Option<HashMap<String, Mat4>>,
        vertex_base: u64,
    ) -> Self {
        Self {
            batches,
            bounds,
            name: name.into(),
            skin_inv_binds,
            transform,
            vertex_base,
        }
    }

    pub(crate) fn batches(&self) -> impl Iterator<Item = Range<u32>> + '_ {
        self.batches.iter().cloned()
    }

    pub fn is_animated(&self) -> bool {
        self.skin_inv_binds.is_some()
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn skin_inv_binds(&self) -> impl Iterator<Item = &Mat4> {
        self.skin_inv_binds.as_ref().unwrap().values()
    }

    pub fn transform(&self) -> Option<Mat4> {
        self.transform
    }

    pub(crate) fn vertex_base(&self) -> u64 {
        self.vertex_base
    }
}

#[derive(Deserialize, Serialize)]
pub struct Model {
    index_ty: IndexType,

    #[serde(with = "serde_bytes")]
    indices: Vec<u8>,

    meshes: Vec<Mesh>,

    #[serde(with = "serde_bytes")]
    vertices: Vec<u8>,
}

impl Model {
    pub(crate) fn new(
        mut meshes: Vec<Mesh>,
        index_ty: IndexType,
        indices: Vec<u8>,
        vertices: Vec<u8>,
    ) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);

        meshes.sort_unstable_by(|lhs, rhs| lhs.name().cmp(&rhs.name()));

        Self {
            index_ty,
            indices,
            meshes,
            vertices,
        }
    }

    pub(crate) fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub(crate) fn index_ty(&self) -> IndexType {
        self.index_ty
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
