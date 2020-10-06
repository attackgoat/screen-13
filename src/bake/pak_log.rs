use {
    super::Asset,
    crate::pak::{BitmapId, ModelId},
    bincode::serialize,
    sha1::Sha1,
    std::collections::HashMap,
};

fn get_key(asset: &Asset) -> [u8; 20] {
    Sha1::from(serialize(asset).unwrap()).digest().bytes()
}

#[derive(Clone)]
pub enum LogId {
    Bitmap(BitmapId),
    Locale(String),
    Model(ModelId),
}

impl From<BitmapId> for LogId {
    fn from(id: BitmapId) -> LogId {
        LogId::Bitmap(id)
    }
}

impl From<ModelId> for LogId {
    fn from(id: ModelId) -> LogId {
        LogId::Model(id)
    }
}

pub struct PakLog {
    ids: HashMap<[u8; 20], LogId>,
}

impl PakLog {
    pub fn add<I: Into<LogId>>(&mut self, asset: &Asset, value: I) {
        self.ids.insert(get_key(asset), value.into());
    }

    pub fn contains(&self, asset: &Asset) -> bool {
        self.get(asset).is_some()
    }

    pub fn get(&self, asset: &Asset) -> Option<LogId> {
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
