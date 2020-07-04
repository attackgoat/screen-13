use {
    super::{BitmapId, DataRef},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Mesh {
    bitmaps: Vec<BitmapId>,
    vertices: DataRef<Vec<u8>>,
}

impl Mesh {
    pub fn new(bitmaps: Vec<BitmapId>, vertices: Vec<u8>) -> Self {
        #[cfg(debug_assertions)]
        assert_ne!(vertices.len(), 0);

        Self {
            bitmaps,
            vertices: DataRef::Data(vertices),
        }
    }

    pub(crate) fn new_ref(bitmaps: Vec<BitmapId>, pos: u32, len: u32) -> Self {
        #[cfg(debug_assertions)]
        assert_ne!(len, 0);

        Self {
            bitmaps,
            vertices: DataRef::Ref((pos, len)),
        }
    }

    pub(crate) fn as_ref(&self) -> (u64, usize) {
        self.vertices.as_ref()
    }

    pub fn bitmaps(&self) -> &[BitmapId] {
        &self.bitmaps
    }

    pub fn vertices(&self) -> &[u8] {
        self.vertices.as_data()
    }
}
