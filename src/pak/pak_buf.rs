use {
    super::{
        anim::Animation, ids::Id, model::Model, AnimationId, BitmapBuf, BitmapFont, BitmapFontId,
        BitmapId, BlobId, Compression, DataRef, MaterialDesc, MaterialId, ModelId, Scene, SceneId,
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

/// Main serialization container for the `.pak` file format.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PakBuf {
    // These fields are handled by bincode serialization as-is
    ids: HashMap<String, Id>,
    localizations: HashMap<String, HashMap<String, String>>,
    materials: Vec<MaterialDesc>,
    texts: HashMap<String, String>,

    // These fields are loaded on demand
    anims: Vec<DataRef<Animation>>,
    bitmap_fonts: Vec<DataRef<BitmapFont>>,
    bitmaps: Vec<DataRef<BitmapBuf>>,
    blobs: Vec<DataRef<Vec<u8>>>,
    models: Vec<DataRef<Model>>,
    scenes: Vec<DataRef<Scene>>,
}

impl PakBuf {
    pub(super) fn animation(&self, id: AnimationId) -> (u64, usize) {
        self.anims[id.0 as usize].pos_len()
    }

    pub(super) fn bitmap(&self, id: BitmapId) -> (u64, usize) {
        self.bitmaps[id.0 as usize].pos_len()
    }

    pub(super) fn bitmap_font(&self, id: BitmapFontId) -> (u64, usize) {
        self.bitmap_fonts[id.0 as usize].pos_len()
    }

    pub(super) fn blob(&self, id: BlobId) -> (u64, usize) {
        self.blobs[id.0 as usize].pos_len()
    }

    pub(super) fn id<K: AsRef<str>>(&self, key: K) -> Option<Id> {
        self.ids.get(key.as_ref()).cloned()
    }

    pub(super) fn material(&self, id: MaterialId) -> MaterialDesc {
        *self.materials.get(id.0 as usize).unwrap()
    }

    pub(super) fn model(&self, id: ModelId) -> (u64, usize) {
        self.models[id.0 as usize].pos_len()
    }

    pub(crate) fn push_animation(&mut self, key: Option<String>, val: Animation) -> AnimationId {
        let id = AnimationId(self.anims.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.anims.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_bitmap(&mut self, key: Option<String>, val: BitmapBuf) -> BitmapId {
        let id = BitmapId(self.bitmaps.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.bitmaps.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_bitmap_font(&mut self, key: Option<String>, val: BitmapFont) -> BitmapFontId {
        let id = BitmapFontId(self.bitmap_fonts.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.bitmap_fonts.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_blob(&mut self, key: Option<String>, val: Vec<u8>) -> BlobId {
        let id = BlobId(self.blobs.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.blobs.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_localization(&mut self, locale: String, texts: HashMap<String, String>) {
        assert!(self.localizations.get(&locale).is_none());

        self.localizations.insert(locale, texts);
    }

    pub(crate) fn push_material(&mut self, key: Option<String>, val: MaterialDesc) -> MaterialId {
        let id = MaterialId(self.materials.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.materials.push(val);

        id
    }

    pub(crate) fn push_model(&mut self, key: Option<String>, val: Model) -> ModelId {
        let id = ModelId(self.models.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.models.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_scene(&mut self, key: Option<String>, val: Scene) -> SceneId {
        let id = SceneId(self.scenes.len() as _);

        if let Some(key) = key {
            assert!(self.ids.get(&key).is_none());

            self.ids.insert(key, id.into());
        }

        self.scenes.push(DataRef::Data(val));

        id
    }

    pub(crate) fn push_text(&mut self, key: String, val: String) {
        assert!(self.texts.get(&key).is_none());

        self.texts.insert(key, val);
    }

    pub(super) fn scene(&self, id: SceneId) -> (u64, usize) {
        self.scenes[id.0 as usize].pos_len()
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

    #[cfg(not(feature = "bake"))]
    #[inline]
    pub(crate) fn write<W: Seek + Write>(
        self,
        writer: W,
        compression: Option<Compression>,
    ) -> Result<(), Error> {
        self.write_impl(writer, compression)
    }

    /// Serializes a `.pak` file buffer into a `Writer` using optional compression.
    #[cfg(feature = "bake")]
    #[inline]
    pub fn write<W: Seek + Write>(
        self,
        writer: W,
        compression: Option<Compression>,
    ) -> Result<(), Error> {
        self.write_impl(writer, compression)
    }

    pub(crate) fn write_impl<W: Seek + Write>(
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
        self.bitmap_fonts = Self::write_refs(&mut writer, self.bitmap_fonts.drain(..), compression);
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
        mut writer: W,
        refs: I,
        compression: Option<Compression>,
    ) -> Vec<DataRef<T>> {
        let mut res = vec![];
        let mut start = current_pos(&mut writer);

        for ref data in refs {
            {
                let data = data.to_vec();
                let mut writer = Compression::writer(compression, &mut writer);
                writer.write_all(&data).unwrap();
            }

            let end = current_pos(&mut writer);
            res.push(DataRef::Ref(start..end));
            start = end;
        }

        res
    }
}
