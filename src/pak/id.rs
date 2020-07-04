use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BitmapId(pub(crate) u16);

impl From<Id> for BitmapId {
    fn from(id: Id) -> Self {
        match id {
            Id::Bitmap(id) => id,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BlobId(pub(crate) u16);

impl From<Id> for BlobId {
    fn from(id: Id) -> Self {
        match id {
            Id::Blob(id) => id,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Id {
    Bitmap(BitmapId),
    Blob(BlobId),
    Mesh(MeshId),
    Scene(SceneId),
}

impl From<BitmapId> for Id {
    fn from(id: BitmapId) -> Self {
        Self::Bitmap(id)
    }
}

impl From<BlobId> for Id {
    fn from(id: BlobId) -> Self {
        Self::Blob(id)
    }
}

impl From<MeshId> for Id {
    fn from(id: MeshId) -> Self {
        Self::Mesh(id)
    }
}

impl From<SceneId> for Id {
    fn from(id: SceneId) -> Self {
        Self::Scene(id)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct MeshId(pub(crate) u16);

impl From<Id> for MeshId {
    fn from(id: Id) -> Self {
        match id {
            Id::Mesh(id) => id,
            _ => panic!(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SceneId(pub(crate) u16);

impl From<Id> for SceneId {
    fn from(id: Id) -> Self {
        match id {
            Id::Scene(id) => id,
            _ => panic!(),
        }
    }
}
