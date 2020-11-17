use {
    super::{BitmapId, DataRef},
    crate::math::Sphere,
    serde::{Deserialize, Serialize},
    std::ops::Range,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    bounds: Sphere,
    indices: Range<u32>,
    name: Option<String>,
    tri_mode: TriangleMode,
}

impl Mesh {
    pub(crate) fn new<N: Into<Option<String>>>(
        bounds: Sphere,
        indices: Range<u32>,
        name: N,
        tri_mode: TriangleMode,
    ) -> Self {
        Self {
            bounds,
            indices,
            name: name.into(),
            tri_mode,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    indices: DataRef<Vec<u8>>,
    meshes: Vec<Mesh>,
    vertices: DataRef<Vec<u8>>,
}

impl Model {
    pub(crate) fn new(meshes: Vec<Mesh>, indices: Vec<u8>, vertices: Vec<u8>) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);

        Self {
            indices: DataRef::Data(indices),
            meshes,
            vertices: DataRef::Data(vertices),
        }
    }

    pub(crate) fn new_ref(meshes: Vec<Mesh>, indices: Range<u32>, vertices: Range<u32>) -> Self {
        assert_ne!(meshes.len(), 0);
        assert_ne!(indices.len(), 0);
        assert_ne!(vertices.len(), 0);

        Self {
            indices: DataRef::Ref(indices),
            meshes,
            vertices: DataRef::Ref(vertices),
        }
    }

    pub(crate) fn indices(&self) -> &[u8] {
        self.indices.data()
    }

    pub(crate) fn indices_pos_len(&self) -> (u64, usize) {
        self.indices.pos_len()
    }

    pub(crate) fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter()
    }

    pub(crate) fn vertices(&self) -> &[u8] {
        self.vertices.data()
    }

    pub(crate) fn vertices_pos_len(&self) -> (u64, usize) {
        self.vertices.pos_len()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TriangleMode {
    Fan,
    List,
    Strip,
}
