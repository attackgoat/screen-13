use {
    super::{
        id::Id, Animation, AnimationId, Bitmap, BitmapId, BlobId, DataRef, Model, ModelId, SceneId,
        SceneRef,
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

#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PakBuf {
    // These fields are handled by bincode serialization as-is
    ids: HashMap<String, Id>,
    localizations: HashMap<String, HashMap<String, String>>,
    scenes: Vec<Vec<SceneRef>>,
    texts: HashMap<String, String>,

    // These fields have special care because bincode doesn't handle byte arrays well (they're slow)
    anims: Vec<Animation>,
    bitmaps: Vec<Bitmap>,
    blobs: Vec<DataRef<Vec<u8>>>,
    models: Vec<Model>,
}

impl PakBuf {
    pub(super) fn anim_ref<K: AsRef<str>>(&self, key: K) -> &Animation {
        let id: AnimationId = self.id(key).into();
        self.anim_id_ref(id)
    }

    pub(super) fn anim_id_ref(&self, id: AnimationId) -> &Animation {
        &self.anims[id.0 as usize]
    }

    pub(super) fn bitmap_ref<K: AsRef<str>>(&self, key: K) -> &Bitmap {
        let id: BitmapId = self.id(key).into();
        self.bitmap_id_ref(id)
    }

    pub(super) fn bitmap_id_ref(&self, id: BitmapId) -> &Bitmap {
        &self.bitmaps[id.0 as usize]
    }

    pub(super) fn blob_pos_len<K: AsRef<str>>(&self, key: K) -> (u64, usize) {
        let id: BlobId = self.id(key).into();
        self.blobs[id.0 as usize].pos_len()
    }

    fn id<K: AsRef<str>>(&self, key: K) -> Id {
        self.ids
            .get(key.as_ref())
            .unwrap_or_else(|| panic!(format!("Key `{}` not found", key.as_ref())))
            .clone()
    }

    pub(super) fn model_ref<K: AsRef<str>>(&self, key: K) -> &Model {
        let id: ModelId = self.id(key).into();
        &self.models[id.0 as usize]
    }

    pub(crate) fn push_animation(&mut self, key: String, value: Animation) -> AnimationId {
        assert!(self.ids.get(&key).is_none());

        let id = AnimationId(self.anims.len() as _);
        self.ids.insert(key, Id::Animation(id));
        self.anims.push(value);

        id
    }

    pub(crate) fn push_bitmap(&mut self, key: String, value: Bitmap) -> BitmapId {
        assert!(self.ids.get(&key).is_none());

        let id = BitmapId(self.bitmaps.len() as _);
        self.ids.insert(key, Id::Bitmap(id));
        self.bitmaps.push(value);

        id
    }

    pub(crate) fn push_blob(&mut self, key: String, value: Vec<u8>) -> BlobId {
        assert!(self.ids.get(&key).is_none());

        let id = BlobId(self.blobs.len() as _);
        self.ids.insert(key, Id::Blob(id));
        self.blobs.push(DataRef::Data(value));

        id
    }

    pub(crate) fn push_localization(&mut self, locale: String, texts: HashMap<String, String>) {
        self.localizations.insert(locale, texts);
    }

    pub(crate) fn push_scene(&mut self, key: String, value: Vec<SceneRef>) -> SceneId {
        assert!(self.ids.get(&key).is_none());

        let id = SceneId(self.scenes.len() as _);
        self.ids.insert(key, Id::Scene(id));
        self.scenes.push(value);

        id
    }

    pub(crate) fn push_model(&mut self, key: String, value: Model) -> ModelId {
        assert!(self.ids.get(&key).is_none());

        let id = ModelId(self.models.len() as _);
        self.ids.insert(key, Id::Model(id));
        self.models.push(value);

        id
    }

    pub(crate) fn push_text(&mut self, key: String, value: String) {
        self.texts.insert(key, value);
    }

    pub(super) fn scene<K: AsRef<str>>(&self, key: K) -> &[SceneRef] {
        let id: SceneId = self.id(key).into();
        self.scenes.get(id.0 as usize).unwrap()
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

    pub(crate) fn write<W: Seek + Write>(mut self, mut writer: &mut W) -> Result<(), Error> {
        let mut start = 4u32;
        let mut bitmaps = vec![];
        let mut blobs = vec![];
        let mut models = vec![];

        #[cfg(debug_assertions)]
        let started = Instant::now();

        // Write a blank spot that we'll use for the skip header later
        writer.write_all(&0u32.to_ne_bytes())?;

        for anim in &self.anims {}

        for bitmap in &self.bitmaps {
            let pixels = bitmap.pixels();
            writer.write_all(pixels).unwrap();

            let end = start + pixels.len() as u32;
            bitmaps.push(Bitmap::new_ref(
                bitmap.fmt(),
                bitmap.width() as u16,
                start..end,
            ));
            start = end;
        }

        for blob in &self.blobs {
            let data = blob.data();
            writer.write_all(data).unwrap();

            let end = start + data.len() as u32;
            blobs.push(DataRef::Ref(start..end));
            start = end;
        }

        for model in &self.models {
            let indices = model.indices();
            writer.write_all(indices).unwrap();

            let vertices = model.vertices();
            writer.write_all(vertices).unwrap();

            let indices_end = start + indices.len() as u32;
            let vertices_end = indices_end + vertices.len() as u32;
            models.push(Model::new_ref(
                model.meshes().map(Clone::clone).collect(),
                start..indices_end,
                indices_end..vertices_end,
            ));
            start = vertices_end;
        }

        // Update these items with the refs we created; saving with bincode was very
        // slow when serializing the byte vectors - that is why those are saved raw.
        self.bitmaps = bitmaps;
        self.blobs = blobs;
        self.models = models;

        // Write the data portion and then re-seek to the beginning to write the skip header
        serialize_into(&mut writer, &self).unwrap(); // TODO unwrap
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&(start as u32 - 4u32).to_ne_bytes())?;

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
}
