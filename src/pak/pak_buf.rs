use {
    super::{
        id::Id, Animation, AnimationId, AnimationKey, Bitmap, BitmapId, BitmapKey, BlobId, BlobKey,
        Compression, DataRef, Material, MaterialId, MaterialKey, Model, ModelId, ModelKey, Scene,
        SceneId, SceneKey,
    },
    bincode::serialize_into,
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        collections::HashMap,
        io::{Error, Seek, SeekFrom, Write},
        u32,
    },
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

// TODO: https://github.com/rust-lang/rust/issues/59359
fn current_pos<S: Seek>(stream: &mut S) -> u32 {
    stream.seek(SeekFrom::Current(0)).unwrap() as u32
}

#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PakBuf {
    // These fields are handled by bincode serialization as-is
    ids: HashMap<String, Id>,
    localizations: HashMap<String, HashMap<String, String>>,
    materials: Vec<Material>,
    texts: HashMap<String, String>,

    // These fields are loaded on demand
    anims: Vec<DataRef<Animation>>,
    bitmaps: Vec<DataRef<Bitmap>>,
    blobs: Vec<DataRef<Vec<u8>>>,
    models: Vec<DataRef<Model>>,
    scenes: Vec<DataRef<Scene>>,
}

impl PakBuf {
    pub(super) fn animation<K: Into<AnimationKey<S>>, S: AsRef<str>>(
        &self,
        key: K,
    ) -> (AnimationId, u64, usize) {
        let id: AnimationId = match key.into() {
            AnimationKey::Id(id) => id,
            AnimationKey::Key(key) => self.id(key).as_animation().unwrap(),
        };
        let (pos, len) = self.anims[id.0 as usize].pos_len();

        (id, pos, len)
    }

    pub(super) fn bitmap<K: Into<BitmapKey<S>>, S: AsRef<str>>(
        &self,
        key: K,
    ) -> (BitmapId, u64, usize) {
        let id: BitmapId = match key.into() {
            BitmapKey::Id(id) => id,
            BitmapKey::Key(key) => self.id(key).as_bitmap().unwrap(),
        };
        let (pos, len) = self.bitmaps[id.0 as usize].pos_len();

        (id, pos, len)
    }

    pub(super) fn blob<K: Into<BlobKey<S>>, S: AsRef<str>>(&self, key: K) -> (BlobId, u64, usize) {
        let id: BlobId = match key.into() {
            BlobKey::Id(id) => id,
            BlobKey::Key(key) => self.id(key).as_blob().unwrap(),
        };
        let (pos, len) = self.blobs[id.0 as usize].pos_len();

        (id, pos, len)
    }

    pub(super) fn id<K: AsRef<str>>(&self, key: K) -> Id {
        self.ids
            .get(key.as_ref())
            .unwrap_or_else(|| panic!(format!("Key `{}` not found", key.as_ref())))
            .clone()
    }

    pub(super) fn material<K: Into<MaterialKey<S>>, S: AsRef<str>>(
        &self,
        key: K,
    ) -> (MaterialId, &Material) {
        let id: MaterialId = match key.into() {
            MaterialKey::Id(id) => id,
            MaterialKey::Key(key) => self.id(key).as_material().unwrap(),
        };
        let material = self.materials.get(id.0 as usize).unwrap();

        (id, material)
    }

    pub(super) fn model<K: Into<ModelKey<S>>, S: AsRef<str>>(
        &self,
        key: K,
    ) -> (ModelId, u64, usize) {
        let id: ModelId = match key.into() {
            ModelKey::Id(id) => id,
            ModelKey::Key(key) => self.id(key).as_model().unwrap(),
        };
        let (pos, len) = self.models[id.0 as usize].pos_len();

        (id, pos, len)
    }

    pub(crate) fn push_animation(&mut self, key: String, val: Animation) -> AnimationId {
        assert!(self.ids.get(&key).is_none());

        let id = AnimationId(self.anims.len() as _);
        self.ids.insert(key, Id::Animation(id));
        self.anims.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_bitmap(&mut self, key: String, val: Bitmap) -> BitmapId {
        assert!(self.ids.get(&key).is_none());

        let id = BitmapId(self.bitmaps.len() as _);
        self.ids.insert(key, Id::Bitmap(id));
        self.bitmaps.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_blob(&mut self, key: String, val: Vec<u8>) -> BlobId {
        assert!(self.ids.get(&key).is_none());

        let id = BlobId(self.blobs.len() as _);
        self.ids.insert(key, Id::Blob(id));
        self.blobs.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_localization(&mut self, locale: String, texts: HashMap<String, String>) {
        self.localizations.insert(locale, texts);
    }

    pub(crate) fn push_scene(&mut self, key: String, val: Scene) -> SceneId {
        assert!(self.ids.get(&key).is_none());

        let id = SceneId(self.scenes.len() as _);
        self.ids.insert(key, Id::Scene(id));
        self.scenes.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_material(&mut self, key: String, val: Material) -> MaterialId {
        assert!(self.ids.get(&key).is_none());

        let id = MaterialId(self.materials.len() as _);
        self.ids.insert(key, Id::Material(id));
        self.materials.push(val);

        id
    }

    pub(crate) fn push_model(&mut self, key: String, val: Model) -> ModelId {
        assert!(self.ids.get(&key).is_none());

        let id = ModelId(self.models.len() as _);
        self.ids.insert(key, Id::Model(id));
        self.models.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_text(&mut self, key: String, val: String) {
        self.texts.insert(key, val);
    }

    pub(super) fn scene<K: Into<SceneKey<S>>, S: AsRef<str>>(
        &self,
        key: K,
    ) -> (SceneId, u64, usize) {
        let id: SceneId = match key.into() {
            SceneKey::Id(id) => id,
            SceneKey::Key(key) => self.id(key).as_scene().unwrap(),
        };
        let (pos, len) = self.scenes[id.0 as usize].pos_len();

        (id, pos, len)
    }

    pub(super) fn text<K: AsRef<str>>(&self, key: K) -> Cow<str> {
        Cow::from(self.texts.get(key.as_ref()).unwrap())
    }

    pub(super) fn text_locale<K: AsRef<str>, L: AsRef<str>>(&self, key: K, locale: L) -> Cow<str> {
        Cow::from(
            self.localizations
                .get(locale.as_ref())
                .unwrap()
                .get(key.as_ref())
                .unwrap(),
        )
    }

    pub(crate) fn write<W: Seek + Write>(
        mut self,
        mut writer: W,
        compression: Option<Compression>,
    ) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        let started = Instant::now();

        // Write a blank spot that we'll use for the skip header later
        writer.write_all(&0u32.to_ne_bytes())?;

        // Write the compression we're going to be using, if any
        serialize_into(&mut writer, &compression).unwrap(); // TODO unwrap

        // Update these items with the refs we created; saving with bincode was very
        // slow when serializing the byte vectors - that is why those are saved raw.
        self.anims = Self::write_refs(&mut writer, self.anims.drain(..), compression);
        self.bitmaps = Self::write_refs(&mut writer, self.bitmaps.drain(..), compression);
        self.blobs = Self::write_refs(&mut writer, self.blobs.drain(..), compression);
        self.models = Self::write_refs(&mut writer, self.models.drain(..), compression);
        self.scenes = Self::write_refs(&mut writer, self.scenes.drain(..), compression);

        // Write the data portion and then re-seek to the beginning to write the skip header
        let skip = current_pos(&mut writer);
        {
            let mut writer = Compression::writer(compression, &mut writer);
            serialize_into(&mut writer, &self).unwrap(); // TODO unwrap
        }

        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&(skip).to_ne_bytes())?;

        #[cfg(debug_assertions)]
        {
            let elapsed = Instant::now() - started;
            if elapsed.as_millis() > 0 {
                info!(
                    "Write pak took {}ms",
                    elapsed.as_millis().to_formatted_string(&Locale::en)
                );
            }
        }

        Ok(())
    }

    fn write_refs<I: Iterator<Item = DataRef<T>>, T: Serialize, W: Seek + Write>(
        mut writer: &mut W,
        refs: I,
        compression: Option<Compression>,
    ) -> Vec<DataRef<T>> {
        let mut res = vec![];
        let mut start = current_pos(writer);

        for ref data in refs {
            {
                let data = data.to_vec();
                let mut writer = Compression::writer(compression, &mut writer);
                writer.write_all(&data).unwrap();
            }

            let end = current_pos(writer);
            res.push(DataRef::Ref(start..end));
            start = end;
        }

        res
    }
}
