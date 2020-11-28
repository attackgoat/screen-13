use {
    super::Asset,
    crate::pak::{AnimationId, BitmapId, FontBitmapId, MaterialId, ModelId},
    bincode::serialize,
    sha1::Sha1,
    std::collections::HashMap,
};

type Hash = [u8; 20];

fn get_key(asset: &Asset) -> Hash {
    Sha1::from(serialize(asset).unwrap()).digest().bytes()
}

#[derive(Clone)]
pub enum Id {
    Animation(AnimationId),
    Bitmap(BitmapId),
    FontBitmap(FontBitmapId),
    Locale(String),
    Material(MaterialId),
    Model(ModelId),
}

impl From<AnimationId> for Id {
    fn from(id: AnimationId) -> Id {
        Id::Animation(id)
    }
}

impl From<BitmapId> for Id {
    fn from(id: BitmapId) -> Id {
        Id::Bitmap(id)
    }
}

impl From<FontBitmapId> for Id {
    fn from(id: FontBitmapId) -> Id {
        Id::FontBitmap(id)
    }
}

impl From<MaterialId> for Id {
    fn from(id: MaterialId) -> Id {
        Id::Material(id)
    }
}

impl From<ModelId> for Id {
    fn from(id: ModelId) -> Id {
        Id::Model(id)
    }
}

pub struct PakLog {
    ids: HashMap<Hash, Id>,
}

impl PakLog {
    pub fn add<I: Into<Id>>(&mut self, asset: &Asset, value: I) {
        self.ids.insert(get_key(asset), value.into());
    }

    pub fn contains(&self, asset: &Asset) -> bool {
        self.get(asset).is_some()
    }

    pub fn get(&self, asset: &Asset) -> Option<Id> {
        self.ids.get(&get_key(asset)).cloned()
    }
}

impl Default for PakLog {
    fn default() -> Self {
        Self {
            ids: HashMap::default(),
        }
    }
}
