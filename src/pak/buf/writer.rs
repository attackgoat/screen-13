use {
    super::{
        AnimationBuf, AnimationId, Asset, BitmapBuf, BitmapFontBuf, BitmapFontId, BitmapId, BlobId,
        Data, DataRef, Id, MaterialId, MaterialInfo, ModelBuf, ModelId, SceneBuf, SceneId,
    },
    crate::pak::compression::Compression,
    log::trace,
    serde::Serialize,
    std::{
        collections::HashMap,
        fs::File,
        io::{BufWriter, Error, ErrorKind, Seek, SeekFrom, Write},
        path::Path,
    },
};

// TODO: https://github.com/rust-lang/rust/issues/59359
fn current_pos(stream: &mut impl Seek) -> Result<u32, Error> {
    Ok(stream.seek(SeekFrom::Current(0))? as _)
}

#[derive(Default)]
pub struct Writer {
    compression: Option<Compression>,
    pub(super) ctx: HashMap<Asset, Id>,
    data: Data,
}

impl Writer {
    pub fn push_animation(&mut self, buf: AnimationBuf, key: Option<String>) -> AnimationId {
        let id = AnimationId(self.data.anims.len());
        self.data.anims.push(DataRef::Data(buf));

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());
            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_bitmap_font(&mut self, buf: BitmapFontBuf, key: Option<String>) -> BitmapFontId {
        let id = BitmapFontId(self.data.bitmap_fonts.len());
        self.data.bitmap_fonts.push(DataRef::Data(buf));

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());

            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_bitmap(&mut self, buf: BitmapBuf, key: Option<String>) -> BitmapId {
        let id = BitmapId(self.data.bitmaps.len());
        self.data.bitmaps.push(DataRef::Data(buf));

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());

            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_blob(&mut self, buf: Vec<u8>, key: Option<String>) -> BlobId {
        let id = BlobId(self.data.blobs.len());
        self.data.blobs.push(DataRef::Data(buf));

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());

            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_material(&mut self, info: MaterialInfo, key: Option<String>) -> MaterialId {
        let id = MaterialId(self.data.materials.len());
        self.data.materials.push(info);

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());

            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_model(&mut self, buf: ModelBuf, key: Option<String>) -> ModelId {
        let id = ModelId(self.data.models.len());
        self.data.models.push(DataRef::Data(buf));

        if let Some(key) = key {
            assert!(self.data.ids.get(&key).is_none());

            self.data.ids.insert(key, id.into());
        }

        id
    }

    pub fn push_scene(&mut self, buf: SceneBuf, key: String) -> SceneId {
        let id = SceneId(self.data.scenes.len());
        self.data.scenes.push(DataRef::Data(buf));

        assert!(self.data.ids.get(&key).is_none());

        self.data.ids.insert(key, id.into());

        id
    }

    pub fn with_compression(&mut self, compression: Compression) -> &mut Self {
        self.compression = Some(compression);
        self
    }

    pub fn with_compression_is(&mut self, compression: Option<Compression>) -> &mut Self {
        self.compression = compression;
        self
    }

    pub fn write(self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.write_data(&mut BufWriter::new(File::create(path)?))
    }

    fn write_data(mut self, mut writer: impl Write + Seek) -> Result<(), Error> {
        // Write a blank spot that we'll use for the skip header later
        writer.write_all(&0u32.to_ne_bytes())?;

        // Write the compression we're going to be using, if any
        bincode::serialize_into(&mut writer, &self.compression)
            .map_err(|_| Error::from(ErrorKind::InvalidData))?;

        // Update these items with the refs we created; saving with bincode was very
        // slow when serializing the byte vectors - that is why those are saved raw.
        trace!("Writing animations");
        Self::write_refs(self.compression, &mut writer, &mut self.data.anims)?;

        trace!("Writing bitmaps");
        Self::write_refs(self.compression, &mut writer, &mut self.data.bitmaps)?;

        trace!("Writing blobs");
        Self::write_refs(self.compression, &mut writer, &mut self.data.blobs)?;

        trace!("Writing bitmap fonts");
        Self::write_refs(self.compression, &mut writer, &mut self.data.bitmap_fonts)?;

        trace!("Writing models");
        Self::write_refs(self.compression, &mut writer, &mut self.data.models)?;

        trace!("Writing scenes");
        Self::write_refs(self.compression, &mut writer, &mut self.data.scenes)?;

        // Write the data portion and then re-seek to the beginning to write the skip header
        let skip = current_pos(&mut writer)?;
        {
            let compressed = if let Some(compressed) = self.compression {
                compressed.new_writer(&mut writer)
            } else {
                Box::new(&mut writer)
            };
            bincode::serialize_into(compressed, &self.data)
                .map_err(|_| Error::from(ErrorKind::InvalidData))?;
        }

        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&(skip).to_ne_bytes())?;

        Ok(())
    }

    fn write_refs<T>(
        compression: Option<Compression>,
        mut writer: impl Seek + Write,
        refs: &mut Vec<DataRef<T>>,
    ) -> Result<(), Error>
    where
        T: Serialize,
    {
        let mut res = vec![];
        let mut start = current_pos(&mut writer)?;

        for (idx, data) in refs.drain(..).map(|data| data.serialize()).enumerate() {
            // Write this data, compressed
            {
                let data = data?;
                let mut compressed = if let Some(compressed) = compression {
                    compressed.new_writer(&mut writer)
                } else {
                    Box::new(&mut writer)
                };
                compressed.write_all(&data)?;
            }

            // Push a ref
            let end = current_pos(&mut writer)?;

            trace!("Index {idx} = {} bytes", end - start);

            res.push(DataRef::<T>::Ref(start..end));
            start = end;
        }

        *refs = res;

        Ok(())
    }
}
