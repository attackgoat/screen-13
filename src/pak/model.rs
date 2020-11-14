use {
    super::{BitmapId, DataRef},
    crate::math::Sphere,
    serde::{Deserialize, Serialize},
    std::ops::Range,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    bounds: Sphere,
    indices: Range<usize>,
    name: Option<String>,
    tri_mode: TriangleMode,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    indices: DataRef<Vec<u8>>,
    meshes: Vec<Mesh>,
    vertices: DataRef<Vec<u8>>,
}

impl Model {
    pub fn new(indices: Vec<u8>, vertices: Vec<u8>) -> Self {
        assert_ne!(vertices.len(), 0);

        // Self {
        //     bounds,
        //     vertices: DataRef::Data(vertices),
        // }

        todo!();
    }

    pub(crate) fn new_ref(bounds: Sphere, pos: u32, len: u32) -> Self {
        assert_ne!(len, 0);

        // Self {
        //     bounds,
        //     vertices: DataRef::Ref((pos, len)),
        // }
        todo!();
    }

    // TODO: Hmmm....
    pub(crate) fn as_ref(&self) -> (u64, usize) {
        self.vertices.as_ref()
    }

    /// Returns the components of a bounding sphere enclosing all vertices of this mesh.
    pub fn bounds(&self) -> Sphere {
        // self.bounds

        todo!();
    }

    pub fn vertices(&self) -> &[u8] {
        self.vertices.as_data()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TriangleMode {
    Fan,
    List,
    Strip,
}
