use {
    glam::Mat4,
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        iter::FromIterator,
        ops::Range,
    },
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum IndexType {
    // U8, requires VK_EXT_index_type_uint8 which has 41% support
    U16,
    U32,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    pub index_count: u32,
    pub index_ty: IndexType,
    pub name: Option<String>,
    pub skin_inv_binds: Option<HashMap<String, Mat4>>,
    pub transform: Option<Mat4>,
    pub vertex_count: u32,
}

impl Mesh {
    pub fn is_animated(&self) -> bool {
        self.skin_inv_binds.is_some()
    }
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("ModelBufMesh")
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ModelBuf {
    #[serde(with = "serde_bytes")]
    indices: Vec<u8>,

    pub meshes: Vec<Mesh>,

    #[serde(with = "serde_bytes")]
    vertices: Vec<u8>,
}

impl ModelBuf {
    pub fn new(
        meshes: impl Into<Vec<Mesh>>,
        indices: impl Into<Vec<u8>>,
        vertices: impl Into<Vec<u8>>,
    ) -> Self {
        let mut meshes = meshes.into();
        let indices = indices.into();
        let vertices = vertices.into();

        assert!(!meshes.is_empty());
        assert!(!indices.is_empty());
        assert!(!vertices.is_empty());

        // Filtering relies on meshes being sorted by name
        meshes.sort_unstable_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        Self {
            indices,
            meshes,
            vertices,
        }
    }

    pub fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub fn vertices(&self) -> &[u8] {
        &self.vertices
    }
}

impl Debug for ModelBuf {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("ModelBuf")
    }
}
