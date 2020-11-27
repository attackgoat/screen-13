pub(crate) mod model;
pub(crate) mod scene;

mod anim;
mod bitmap;
mod data_ref;
mod id;
mod material;
mod pak_buf;

// TODO: Remove ErrorKind!
pub use {
    self::{
        anim::{Animation, Channel},
        bitmap::{Bitmap, Format as BitmapFormat},
        id::{AnimationId, BitmapId, BlobId, MaterialId, ModelId, SceneId, TextId},
        material::Material,
        model::Model,
        pak_buf::PakBuf,
        scene::Scene,
    },
    bincode::ErrorKind,
};

use {
    bincode::deserialize_from,
    brotli::{CompressorReader as BrotliReader, CompressorWriter as BrotliWriter},
    serde::{Deserialize, Serialize},
    snap::{read::FrameDecoder as SnapReader, write::FrameEncoder as SnapWriter},
    std::{
        borrow::Cow,
        env::current_exe,
        fs::File,
        io::{BufReader, Cursor, Error, Read, Seek, SeekFrom, Write},
        path::Path,
    },
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

pub(self) use self::data_ref::DataRef;

fn read_exact<R: Read + Seek>(reader: &mut R, pos: u64, len: usize) -> Vec<u8> {
    // Unsafely create a buffer of uninitialized data (this is faster)
    let mut buf = Vec::with_capacity(len);
    unsafe {
        buf.set_len(len);
    }

    // Read the data into our buffer
    reader.seek(SeekFrom::Start(pos)).unwrap(); // TODO: Unwrapping IO reads!!
    reader.read_exact(&mut buf).unwrap();

    buf
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(crate) struct BrotliCompression {
    pub buf_size: usize,
    pub quality: u32,
    pub window_size: u32,
}

impl Default for BrotliCompression {
    fn default() -> Self {
        Self {
            buf_size: 4096,
            quality: 10,
            window_size: 20,
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(crate) enum Compression {
    Brotli(BrotliCompression),
    Snap,
}

impl Compression {
    fn reader<'r, R: Read + 'r>(compression: Option<Self>, reader: R) -> Box<dyn Read + 'r> {
        match compression {
            Some(compression) => match compression {
                Compression::Brotli(b) => Box::new(BrotliReader::new(
                    reader,
                    b.buf_size,
                    b.quality,
                    b.window_size,
                )),
                Compression::Snap => Box::new(SnapReader::new(reader)),
            },
            _ => Box::new(reader),
        }
    }

    fn writer<'w, W: Write + 'w>(compression: Option<Self>, writer: W) -> Box<dyn Write + 'w> {
        match compression {
            Some(compression) => match compression {
                Compression::Brotli(b) => Box::new(BrotliWriter::new(
                    writer,
                    b.buf_size,
                    b.quality,
                    b.window_size,
                )),
                Compression::Snap => Box::new(SnapWriter::new(writer)),
            },
            _ => Box::new(writer),
        }
    }
}

impl Default for Compression {
    fn default() -> Self {
        Self::Brotli(Default::default())
    }
}

pub struct Pak<R>
where
    R: Read + Seek,
{
    buf: PakBuf,
    reader: R,
}

impl Pak<BufReader<File>> {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let current_dir = current_exe()?.parent().unwrap().to_path_buf(); // TODO: Unwrap
        let pak_path = current_dir.join(&path);
        let pak_file = File::open(&pak_path)?;
        let mut reader = BufReader::new(pak_file);

        #[cfg(debug_assertions)]
        let started = Instant::now();

        let skip = {
            let mut buf: [u8; 4] = Default::default();
            reader.read_exact(&mut buf).unwrap();
            u32::from_ne_bytes(buf)
        };

        let compression: Option<Compression> = deserialize_from(&mut reader).unwrap();

        reader.seek(SeekFrom::Current(skip as _))?;

        let buf = {
            let mut reader = Compression::reader(compression, &mut reader);
            deserialize_from(&mut reader).unwrap()
        };

        #[cfg(debug_assertions)]
        {
            let elapsed = Instant::now() - started;
            if elapsed.as_millis() > 0 {
                info!(
                    "PakBuf::open took {}ms",
                    elapsed.as_millis().to_formatted_string(&Locale::en)
                );
            }
        }

        Ok(Self { buf, reader })
    }
}

impl<R> Pak<R>
where
    R: Read + Seek,
{
    pub fn animation_id<K: AsRef<str>>(&self, key: K) -> Option<AnimationId> {
        self.buf.id(key).as_animation()
    }

    pub fn bitmap_id<K: AsRef<str>>(&self, key: K) -> Option<BitmapId> {
        self.buf.id(key).as_bitmap()
    }

    pub fn blob_id<K: AsRef<str>>(&self, key: K) -> Option<BlobId> {
        self.buf.id(key).as_blob()
    }

    pub fn material_id<K: AsRef<str>>(&self, key: K) -> Option<MaterialId> {
        self.buf.id(key).as_material()
    }

    pub fn material(&self, id: MaterialId) -> &Material {
        self.buf.material(id)
    }

    pub fn model_id<K: AsRef<str>>(&self, key: K) -> Option<ModelId> {
        self.buf.id(key).as_model()
    }

    pub fn scene_id<K: AsRef<str>>(&self, key: K) -> Option<SceneId> {
        self.buf.id(key).as_scene()
    }

    pub fn text<K: AsRef<str>>(&self, key: K) -> Cow<str> {
        // TODO: Pick proper user locale or best guess; use additional libs to detect!
        self.buf.text_locale(key, "en-US")
    }

    pub fn text_id<K: AsRef<str>>(&self, key: K) -> Option<TextId> {
        self.buf.id(key).as_text()
    }

    pub fn text_locale<K: AsRef<str>, L: AsRef<str>>(&self, key: K, locale: L) -> Cow<str> {
        self.buf.text_locale(key, locale)
    }

    pub fn text_raw<K: AsRef<str>>(&self, key: K) -> Cow<str> {
        self.buf.text(key)
    }

    pub fn read_animation(&mut self, id: AnimationId) -> Animation {
        let (pos, len) = self.buf.animation(id);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }

    pub fn read_blob(&mut self, id: BlobId) -> Vec<u8> {
        let (pos, len) = self.buf.blob(id);

        read_exact(&mut self.reader, pos, len)
    }

    pub fn read_bitmap(&mut self, id: BitmapId) -> Bitmap {
        let (pos, len) = self.buf.bitmap(id);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }

    pub fn read_model(&mut self, id: ModelId) -> Model {
        let (pos, len) = self.buf.model(id);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }

    pub fn read_scene(&mut self, id: SceneId) -> Scene {
        let (pos, len) = self.buf.scene(id);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }
}
