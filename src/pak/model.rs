use {
    super::{BitmapId, DataRef},
    crate::math::Sphere,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    bounds: Sphere,
    bitmaps: Vec<BitmapId>,
    vertices: DataRef<Vec<u8>>,
}

impl Model {
    pub fn new(bitmaps: Vec<BitmapId>, bounds: Sphere, vertices: Vec<u8>) -> Self {
        assert_ne!(vertices.len(), 0);

        Self {
            bitmaps,
            bounds,
            vertices: DataRef::Data(vertices),
        }
    }

    pub(crate) fn new_ref(bitmaps: Vec<BitmapId>, bounds: Sphere, pos: u32, len: u32) -> Self {
        assert_ne!(len, 0);

        Self {
            bitmaps,
            bounds,
            vertices: DataRef::Ref((pos, len)),
        }
    }

    // TODO: Hmmm....
    pub(crate) fn as_ref(&self) -> (u64, usize) {
        self.vertices.as_ref()
    }

    pub fn bitmaps(&self) -> &[BitmapId] {
        &self.bitmaps
    }

    /// Returns the components of a bounding sphere enclosing all vertices of this mesh.
    pub fn bounds(&self) -> Sphere {
        self.bounds
    }

    pub fn vertices(&self) -> &[u8] {
        self.vertices.as_data()
    }
}
