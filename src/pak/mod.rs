mod anim;
mod bitmap;
mod data_ref;
mod id;
mod model;
mod pak_buf;
mod scene_ref;

// TODO: Remove ErrorKind!
pub use {
    self::{
        anim::{Animation, Channel},
        bitmap::{Bitmap, Format as BitmapFormat},
        id::{AnimationId, BitmapId, BlobId, ModelId, SceneId},
        model::{Mesh, Model, TriangleMode},
        pak_buf::PakBuf,
        scene_ref::SceneRef,
    },
    bincode::ErrorKind,
};

use {
    bincode::deserialize_from,
    std::{
        borrow::Cow,
        env::current_exe,
        fs::File,
        io::{BufReader, Cursor, Error, Read, Seek, SeekFrom},
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
        reader.seek(SeekFrom::Current(skip as _))?;
        let buf = deserialize_from(&mut reader).unwrap();

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
    pub fn scene<K: AsRef<str>>(&self, key: K) -> &[SceneRef] {
        self.buf.scene(key)
    }

    pub fn text<K: AsRef<str>>(&self, key: K) -> Cow<str> {
        // TODO: Pick proper user locale or best guess; use additional libs to detect!
        self.buf.text_locale(key, "en-US")
    }

    pub fn text_locale<K: AsRef<str>, L: AsRef<str>>(&self, key: K, locale: L) -> Cow<str> {
        self.buf.text_locale(key, locale)
    }

    pub fn text_raw<K: AsRef<str>>(&self, key: K) -> Cow<str> {
        self.buf.text(key)
    }

    pub(crate) fn read_animation<K: AsRef<str>>(&mut self, key: K) -> Animation {
        let (pos, len) = self.buf.animation(key);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }

    pub fn read_blob<K: AsRef<str>>(&mut self, key: K) -> Vec<u8> {
        let (pos, len) = self.buf.blob_pos_len(key);

        read_exact(&mut self.reader, pos, len)
    }

    pub(crate) fn read_bitmap<K: AsRef<str>>(&mut self, key: K) -> Bitmap {
        let (pos, len) = self.buf.bitmap(key);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }

    pub(crate) fn read_model<K: AsRef<str>>(&mut self, key: K) -> Model {
        let (pos, len) = self.buf.model(key);
        let buf = read_exact(&mut self.reader, pos, len);
        let mut reader = Cursor::new(buf);

        deserialize_from(&mut reader).unwrap()
    }
}
