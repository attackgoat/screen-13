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
    base_vertex: Option<u32>,
    bounds: Sphere,
    indices: Range<u32>,
    name: Option<String>,
    skin_inv_binds: Option<HashMap<String, Mat4>>,
    transform: Option<Mat4>,
    vertex_count: u32,
    vertex_offset: u32,
}

impl Mesh {
    pub fn new<N: Into<Option<String>>>(
        name: N,
        indices: Range<u32>,
        vertex_count: u32,
        vertex_offset: u32,
        bounds: Sphere,
        transform: Option<Mat4>,
        skin_inv_binds: Option<HashMap<String, Mat4>>,
    ) -> Self {
        Self {
            base_vertex: None,
            bounds,
            indices,
            name: name.into(),
            skin_inv_binds,
            transform,
            vertex_count,
            vertex_offset,
        }
    }

    // The number of (same sized) vertices that appear before this one in the vertex buffer, by simple
    // division of the position and stride of the vertices of this mesh.
    pub fn base_vertex(&self) -> u32 {
        self.base_vertex.unwrap()
    }

    pub(crate) fn indices(&self) -> Range<u32> {
        self.indices.clone()
    }

    pub fn is_animated(&self) -> bool {
        self.skin_inv_binds.is_some()
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn set_base_vertex(&mut self, val: u32) {
        self.base_vertex = Some(val);
    }

    pub(crate) fn skin_inv_binds(&self) -> impl Iterator<Item = &Mat4> {
        self.skin_inv_binds.as_ref().unwrap().values()
    }

    pub fn transform(&self) -> Option<Mat4> {
        self.transform
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Offset in the vertex buffer, in bytes, where our first vertex begins
    pub fn vertex_offset(&self) -> u32 {
        self.vertex_offset
    }
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Mesh")
    }
}

#[derive(Deserialize, Serialize)]
pub struct Model {
    idx_ty: IndexType,

    #[serde(with = "serde_bytes")]
    indices: Vec<u8>,

    meshes: Vec<Mesh>,

    #[serde(with = "serde_bytes")]
    vertices: Vec<u8>,

    #[serde(with = "serde_bytes")]
    write_mask: Vec<u8>,
}

impl Model {
    pub(crate) fn new(
        mut meshes: Vec<Mesh>,
        idx_ty: IndexType,
        indices: Vec<u8>,
        vertices: Vec<u8>,
        write_mask: Vec<u8>,
    ) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);
        assert_ne!(write_mask.len(), 0);

        // Filtering relies on meshes being sorted by name
        meshes.sort_unstable_by(|lhs, rhs| lhs.name().cmp(&rhs.name()));

        Self {
            idx_ty,
            indices,
            meshes,
            vertices,
            write_mask,
        }
    }

    pub(crate) fn idx_ty(&self) -> IndexType {
        self.idx_ty
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

    pub(crate) fn write_mask(&self) -> &[u8] {
        &self.write_mask
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}
