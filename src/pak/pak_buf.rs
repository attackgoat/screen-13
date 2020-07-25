use {
    super::{id::Id, Bitmap, BitmapId, BlobId, DataRef, Mesh, MeshId, SceneId, SceneRef},
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

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug)]
pub struct PakBuf {
    // These fields are handled by bincode serialization as-is
    ids: HashMap<String, Id>,
    localizations: HashMap<String, HashMap<String, String>>,
    scenes: Vec<Vec<SceneRef>>,
    texts: HashMap<String, String>,

    // These fields have special care because bincode doesn't handle byte arrays well (they're slow)
    bitmaps: Vec<Bitmap>,
    blobs: Vec<DataRef<Vec<u8>>>,
    meshes: Vec<Mesh>,
}

impl PakBuf {
    pub(super) fn bitmap_ref<K: AsRef<str>>(&self, key: K) -> &Bitmap {
        let id: BitmapId = self.id(key).into();
        self.bitmap_id_ref(id)
    }

    pub(super) fn bitmap_id_ref(&self, id: BitmapId) -> &Bitmap {
        &self.bitmaps[id.0 as usize]
    }

    pub(super) fn blob_ref<K: AsRef<str>>(&self, key: K) -> (u64, usize) {
        let id: BlobId = self.id(key).into();
        self.blobs[id.0 as usize].as_ref()
    }

    fn id<K: AsRef<str>>(&self, key: K) -> Id {
        self.ids
            .get(key.as_ref())
            .unwrap_or_else(|| panic!(format!("Key `{}` not found", key.as_ref())))
            .clone()
    }

    pub(super) fn mesh_ref<K: AsRef<str>>(&self, key: K) -> &Mesh {
        let id: MeshId = self.id(key).into();
        &self.meshes[id.0 as usize]
    }

    pub fn push_bitmap(&mut self, key: String, value: Bitmap) -> BitmapId {
        assert!(self.ids.get(&key).is_none());

        let id = BitmapId(self.bitmaps.len() as _);
        self.ids.insert(key, Id::Bitmap(id));
        self.bitmaps.push(value);

        id
    }

    pub fn push_blob(&mut self, key: String, value: Vec<u8>) -> BlobId {
        assert!(self.ids.get(&key).is_none());

        let id = BlobId(self.blobs.len() as _);
        self.ids.insert(key, Id::Blob(id));
        self.blobs.push(DataRef::Data(value));

        id
    }

    pub fn push_localization(&mut self, locale: String, texts: HashMap<String, String>) {
        self.localizations.insert(locale, texts);
    }

    pub fn push_scene(&mut self, key: String, value: Vec<SceneRef>) -> SceneId {
        assert!(self.ids.get(&key).is_none());

        let id = SceneId(self.scenes.len() as _);
        self.ids.insert(key, Id::Scene(id));
        self.scenes.push(value);

        id
    }

    pub fn push_mesh(&mut self, key: String, value: Mesh) -> MeshId {
        assert!(self.ids.get(&key).is_none());

        let id = MeshId(self.meshes.len() as _);
        self.ids.insert(key, Id::Mesh(id));
        self.meshes.push(value);

        id
    }

    pub fn push_text(&mut self, key: String, value: String) {
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

    pub fn write<W: Seek + Write>(mut self, mut writer: &mut W) -> Result<(), Error> {
        let mut skip = 0u32;
        let mut pos = 4;
        let mut bitmaps = vec![];
        let mut blobs = vec![];
        let mut meshes = vec![];

        #[cfg(debug_assertions)]
        let started = Instant::now();

        // Write a blank spot that we'll use for the skip header later
        writer.write_all(&skip.to_ne_bytes())?;

        for bitmap in &self.bitmaps {
            let pixels = bitmap.pixels();
            let len = pixels.len() as u32;
            writer.write_all(pixels).unwrap();
            bitmaps.push(Bitmap::new_ref(
                bitmap.has_alpha(),
                bitmap.width() as u16,
                pos,
                len,
            ));

            pos += len;
            skip += len;
        }

        for blob in &self.blobs {
            let data = blob.as_data();
            let len = data.len() as u32;
            writer.write_all(data).unwrap();
            blobs.push(DataRef::Ref((pos, len)));

            pos += len;
            skip += len;
        }

        for mesh in &self.meshes {
            let bounds = mesh.bounds();
            let vertices = mesh.vertices();
            let len = vertices.len() as u32;
            writer.write_all(vertices).unwrap();
            meshes.push(Mesh::new_ref(mesh.bitmaps().to_vec(), bounds, pos, len));
            pos += len;
            skip += len;
        }

        // Update these items with the refs we created; saving with bincode was very
        // slow when serializing the byte vectors - that is why those are saved raw.
        self.bitmaps = bitmaps;
        self.blobs = blobs;
        self.meshes = meshes;

        // Write the data portion and then re-seek to the beginning to write the skip header
        serialize_into(&mut writer, &self).unwrap(); // TODO unwrap
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&skip.to_ne_bytes())?;

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
