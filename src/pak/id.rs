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
    Model(ModelId),
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

impl From<ModelId> for Id {
    fn from(id: ModelId) -> Self {
        Self::Model(id)
    }
}

impl From<SceneId> for Id {
    fn from(id: SceneId) -> Self {
        Self::Scene(id)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ModelId(pub(crate) u16);

impl From<Id> for ModelId {
    fn from(id: Id) -> Self {
        match id {
            Id::Model(id) => id,
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
