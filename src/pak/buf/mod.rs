//! Contains functions and types used to bake assets into .pak files
//!
//! Assets are regular art such as `.glb`, `.jpeg` and `.ttf` files.

mod anim;
mod asset;
mod bitmap;
mod blob;
mod content;
mod material;
mod model;
mod scene;

#[cfg(feature = "bake")]
mod writer;

#[cfg(feature = "bake")]
pub use self::writer::Writer;

use {
    self::{asset::Asset, bitmap::Bitmap, blob::Blob, model::Model},
    super::{
        compression::Compression, AnimationBuf, AnimationId, BitmapBuf, BitmapFontBuf,
        BitmapFontId, BitmapId, BlobId, MaterialId, MaterialInfo, ModelBuf, ModelId, Pak, SceneBuf,
        SceneId,
    },
    log::{error, info, trace, warn},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::{
        collections::HashMap,
        env::var,
        fmt::{Debug, Formatter},
        fs::File,
        io::{BufReader, Cursor, Error, ErrorKind, Read, Seek, SeekFrom},
        ops::Range,
        path::{Path, PathBuf},
        u32,
    },
};

#[cfg(feature = "bake")]
use {anyhow::Context, glob::glob, parking_lot::Mutex, std::sync::Arc, tokio::runtime::Runtime};

/// Given some parent directory and a filename, returns just the portion after the directory.
#[allow(unused)]
fn file_key(dir: impl AsRef<Path>, path: impl AsRef<Path>) -> String {
    let res_dir = dir.as_ref();
    let mut path = path.as_ref();
    let mut parts = vec![];

    while path != res_dir {
        {
            let path = path.file_name();
            if path.is_none() {
                break;
            }

            let path = path.unwrap();
            let path_str = path.to_str();
            if path_str.is_none() {
                break;
            }

            parts.push(path_str.unwrap().to_string());
        }
        path = path.parent().unwrap();
    }

    let mut key = String::new();
    for part in parts.iter().rev() {
        if !key.is_empty() {
            key.push('/');
        }

        key.push_str(part);
    }

    // Strip off the toml extension as needed
    let mut key = PathBuf::from(key);
    if is_toml(&key) {
        key = key.with_extension("");
    }

    key.to_str().unwrap().to_owned()
}

fn is_cargo_build() -> bool {
    var("CARGO").is_ok()
}

/// Returns `true` when a given path has the `.toml` file extension.
fn is_toml(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| *ext == "toml")
        .is_some()
}

/// Returns either the parent directory of the given path or the project root if the path has no
/// parent.
fn parent(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref()
        .parent()
        .map(|path| path.to_owned())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn parse_hex_color(val: &str) -> Option<[u8; 4]> {
    let mut res = [u8::MAX; 4];
    let len = val.len();
    match len {
        4 | 5 => {
            res[0] = u8::from_str_radix(&val[1..2].repeat(2), 16).unwrap();
            res[1] = u8::from_str_radix(&val[2..3].repeat(2), 16).unwrap();
            res[2] = u8::from_str_radix(&val[3..4].repeat(2), 16).unwrap();
        }
        7 | 9 => {
            res[0] = u8::from_str_radix(&val[1..3], 16).unwrap();
            res[1] = u8::from_str_radix(&val[3..5], 16).unwrap();
            res[2] = u8::from_str_radix(&val[5..7], 16).unwrap();
        }
        _ => return None,
    }

    match len {
        5 => res[3] = u8::from_str_radix(&val[4..5].repeat(2), 16).unwrap(),
        9 => res[3] = u8::from_str_radix(&val[7..9], 16).unwrap(),
        _ => unreachable!(),
    }

    Some(res)
}

fn parse_hex_scalar(val: &str) -> Option<u8> {
    match val.len() {
        2 => Some(u8::from_str_radix(&val[1..2].repeat(2), 16).unwrap()),
        3 => Some(u8::from_str_radix(&val[1..3], 16).unwrap()),
        _ => None,
    }
}

fn re_run_if_changed(p: impl AsRef<Path>) {
    if is_cargo_build() {
        println!("cargo:rerun-if-changed={}", p.as_ref().display());
    }
}

trait Canonicalize {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>);

    /// Gets the fully rooted source path.
    ///
    /// If `src` is relative, then `src_dir` is used to determine the relative parent.
    /// If `src` is absolute, then `project_dir` is considered to be its root.
    fn canonicalize_project_path(
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
        src: impl AsRef<Path>,
    ) -> PathBuf {
        //trace!("Getting path for {} in {} (res_dir={})", path.as_ref().display(), path_dir.as_ref().display(), res_dir.as_ref().display());

        // Absolute paths are 'project aka resource directory' absolute, not *your host file system*
        // absolute!
        if src.as_ref().is_absolute() {
            // TODO: This could be way simpler!

            // Build an array of path items (file and directories) until the root
            let mut temp = Some(src.as_ref());
            let mut parts = vec![];
            while let Some(path) = temp {
                if let Some(part) = path.file_name() {
                    parts.push(part);
                    temp = path.parent();
                } else {
                    break;
                }
            }

            // Paste the incoming path (minus root) onto the res_dir parameter
            let mut temp = project_dir.as_ref().to_path_buf();
            for part in parts.iter().rev() {
                temp = temp.join(part);
            }

            temp.canonicalize().unwrap_or_else(|_| {
                error!(
                    "Unable to canonicalize {} with {} ({})",
                    project_dir.as_ref().display(),
                    src.as_ref().display(),
                    temp.display(),
                );
                panic!("{} not found", temp.display());
            })
        } else {
            let temp = src_dir.as_ref().join(&src);
            temp.canonicalize().unwrap_or_else(|_| {
                error!(
                    "Unable to canonicalize {} with {} ({})",
                    src_dir.as_ref().display(),
                    src.as_ref().display(),
                    temp.display(),
                );
                panic!("{} not found", temp.display());
            })
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Data {
    // These fields are handled by bincode serialization as-is
    ids: HashMap<String, Id>,
    materials: Vec<MaterialInfo>,

    // These fields are loaded on demand
    anims: Vec<DataRef<AnimationBuf>>,
    bitmap_fonts: Vec<DataRef<BitmapFontBuf>>,
    bitmaps: Vec<DataRef<BitmapBuf>>,
    blobs: Vec<DataRef<Vec<u8>>>,
    models: Vec<DataRef<ModelBuf>>,
    scenes: Vec<DataRef<SceneBuf>>,
}

#[derive(Deserialize, PartialEq, Serialize)]
enum DataRef<T> {
    Data(T),
    Ref(Range<u32>),
}

impl<T> DataRef<T> {
    fn as_data(&self) -> Option<&T> {
        match self {
            Self::Data(ref t) => Some(t),
            _ => {
                warn!("Expected data but found position and length");

                None
            }
        }
    }

    fn is_data(&self) -> bool {
        matches!(self, Self::Data(_))
    }

    fn pos_len(&self) -> Option<(u64, usize)> {
        match self {
            Self::Ref(range) => Some((range.start as _, (range.end - range.start) as _)),
            _ => {
                warn!("Expected position and length but found data");

                None
            }
        }
    }
}

impl<T> DataRef<T>
where
    T: Serialize,
{
    fn serialize(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![];
        bincode::serialize_into(&mut buf, &self.as_data().unwrap())
            .map_err(|_| Error::from(ErrorKind::InvalidData))?;

        Ok(buf)
    }
}

impl<T> Debug for DataRef<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Data(_) => "Data",
            Self::Ref(_) => "DataRef",
        })
    }
}

macro_rules! id_enum {
    ($($variant:ident),*) => {
        paste::paste! {
            #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
            enum Id {
                $(
                    $variant([<$variant Id>]),
                )*
            }

            impl Id {
                $(
                    fn [<as_ $variant:snake>](&self) -> Option<[<$variant Id>]> {
                        match self {
                            Self::$variant(id) => Some(*id),
                            _ => None,
                        }
                    }
                )*
            }

            $(
                impl From<[<$variant Id>]> for Id {
                    fn from(id: [<$variant Id>]) -> Self {
                        Self::$variant(id)
                    }
                }
            )*
        }
    };
}

id_enum!(Animation, Bitmap, BitmapFont, Blob, Material, Model, Scene);

/// Main serialization container for the `.pak` file format.
#[derive(Debug)]
pub struct PakBuf {
    compression: Option<Compression>,
    data: Data,
    reader: Box<dyn Stream>,
}

impl PakBuf {
    #[cfg(feature = "bake")]
    pub fn bake(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> anyhow::Result<()> {
        re_run_if_changed(&src);

        let rt = Arc::new(Runtime::new()?);
        let mut tasks = vec![];
        let mut writer = Arc::new(Mutex::new(Default::default()));

        // Load the source file into an Asset::Content instance
        let src_dir = parent(&src);
        let content = Asset::read(&src)?
            .into_content()
            .context("Unable to read asset file")?;

        // Process each file we find as a separate runtime task
        for asset_glob in content
            .groups()
            .into_iter()
            .filter(|group| group.enabled())
            .flat_map(|group| group.asset_globs())
        {
            let asset_paths = glob(src_dir.join(asset_glob).to_string_lossy().as_ref())
                .context("Unable to glob source directory")?;
            for asset_path in asset_paths {
                let asset_path = asset_path.context("Unable to get asset path")?;

                re_run_if_changed(&asset_path);

                match asset_path
                    .extension()
                    .map(|ext| ext.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .to_lowercase()
                    .as_str()
                {
                    "glb" | "gltf" => {
                        // Note that direct references like this build a model, not an animation
                        // To build an animation you must specify a .toml file
                        let writer = Arc::clone(&writer);
                        let src_dir = src_dir.clone();
                        let asset_path = asset_path.clone();
                        tasks.push(rt.spawn_blocking(move || {
                            Model::new(&asset_path)
                                .bake(&writer, &src_dir, Some(&asset_path))
                                .unwrap();
                        }));
                    }
                    "jpg" | "jpeg" | "png" | "bmp" | "tga" | "dds" | "webp" | "gif" | "ico"
                    | "tiff" => {
                        let writer = Arc::clone(&writer);
                        let src_dir = src_dir.clone();
                        let asset_path = asset_path.clone();
                        tasks.push(rt.spawn_blocking(move || {
                            Bitmap::new(&asset_path)
                                .bake_from_source(&writer, &src_dir, Some(&asset_path))
                                .unwrap();
                        }));
                    }
                    "toml" => {
                        let asset = Asset::read(&asset_path)?;
                        let asset_parent = parent(&asset_path);

                        match asset {
                            //     Asset::Animation(anim) => {
                            //         // bake_animation(&mut context, &src_dir, asset_filename, anim, &mut pak);
                            //         todo!();
                            //     }
                            //     // Asset::Atlas(ref atlas) => {
                            //     //     bake_atlas(&src_dir, &asset_filename, atlas, &mut pak);
                            //     // }
                            // Asset::Bitmap(mut bitmap) => {
                            //     bitmap.canonicalize(&src_dir, &src_dir);
                            //     bake_bitmap(&mut context, &mut pak, &src_dir, Some(src), &bitmap);
                            // }
                            //     Asset::BitmapFont(mut bitmap_font) => {
                            //         bitmap_font.canonicalize(&src_dir, &src_dir);
                            //         bake_bitmap_font(&mut context, &mut pak, src_dir, src, bitmap_font);
                            //     }
                            //     Asset::Color(_) => unreachable!(),
                            //     Asset::Content(_) => {
                            //         // Nested content files are not yet supported
                            //         panic!("Unexpected content file {}", src.display());
                            //     }
                            //     // Asset::Language(ref lang) => {
                            //     //     bake_lang(&src_dir, &asset_filename, lang, &mut pak, &mut log)
                            //     // }
                            Asset::Material(mut material) => {
                                let writer = Arc::clone(&writer);
                                let src_dir = src_dir.clone();
                                let asset_path = asset_path.clone();
                                let asset_parent = asset_parent.clone();
                                let rt2 = rt.clone();
                                tasks.push(rt.spawn_blocking(move || {
                                    material.canonicalize(&src_dir, &asset_parent);
                                    material
                                        .bake(
                                            &rt2,
                                            &writer,
                                            &src_dir,
                                            &asset_parent,
                                            Some(&asset_path),
                                        )
                                        .unwrap();
                                }));
                            }
                            //     Asset::Model(mut model) => {
                            //         model.canonicalize(&src_dir, &src_dir);
                            //         bake_model(&mut context, &mut pak, src_dir, Some(src), &model);
                            //     }
                            //     Asset::Scene(scene) => {
                            //         bake_scene(&mut context, &mut pak, &src_dir, src, &scene);
                            //     }
                            _ => unimplemented!(),
                        }
                    }
                    _ => {
                        let writer = Arc::clone(&writer);
                        let src_dir = src_dir.clone();
                        let asset_path = asset_path.clone();
                        tasks.push(
                            rt.spawn_blocking(move || Blob::bake(&writer, &src_dir, &asset_path)),
                        );
                    }
                }
            }
        }

        rt.block_on(async move {
            for task in tasks.into_iter() {
                task.await.unwrap();
            }

            writer.lock().write(dst).unwrap();
        });

        Ok(())
    }

    fn deserialize<T>(&mut self, pos: u64, len: usize) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        trace!("Read data: {len} bytes");

        // Create a zero-filled buffer
        let mut buf = vec![0; len];

        // Read the data into our buffer
        self.reader.seek(SeekFrom::Start(pos))?;
        self.reader.read_exact(&mut buf)?;
        let data = buf.as_slice();

        // Optionally create a compression reader (or just use the one we have)
        if let Some(compressed) = self.compression {
            bincode::deserialize_from(compressed.new_reader(data))
        } else {
            bincode::deserialize_from(data)
        }
        .map_err(|err| Error::from(ErrorKind::InvalidData))
    }

    pub fn from_stream(mut stream: impl Stream + 'static) -> Result<Self, Error> {
        // Read the number of bytes we must 'skip' in order to read the main data
        let skip = {
            let mut buf: [u8; 4] = Default::default();
            stream.read_exact(&mut buf)?;
            u32::from_ne_bytes(buf)
        };
        let compression: Option<Compression> = bincode::deserialize_from(&mut stream)
            .map_err(|_| Error::from(ErrorKind::InvalidData))?;

        // Read the compressed main data
        stream.seek(SeekFrom::Start(skip as _))?;
        let data: Data = {
            let mut compressed = if let Some(compressed) = compression {
                compressed.new_reader(&mut stream)
            } else {
                Box::new(&mut stream)
            };
            bincode::deserialize_from(&mut compressed)
                .map_err(|_| Error::from(ErrorKind::InvalidData))?
        };

        trace!(
            "Read header: {} bytes ({} keys)",
            stream.stream_position()? - skip as u64,
            data.ids.len()
        );

        Ok(Self {
            compression,
            data,
            reader: Box::new(stream),
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.data.ids.keys().map(|key| key.as_str())
    }

    /// Opens the given path and decodes a `Pak`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let buf = BufReader::new(file);

        Self::from_stream(PakFile { buf, path })
    }
}

impl Pak for PakBuf {
    /// Gets the pak-unique `AnimationId` corresponding to the given key, if one exsits.
    fn animation_id(&self, key: impl AsRef<str>) -> Option<AnimationId> {
        self.data
            .ids
            .get(key.as_ref())
            .and_then(|id| id.as_animation())
    }

    /// Gets the pak-unique `BitmapFontId` corresponding to the given key, if one exsits.
    fn bitmap_font_id(&self, key: impl AsRef<str>) -> Option<BitmapFontId> {
        self.data
            .ids
            .get(key.as_ref())
            .and_then(|id| id.as_bitmap_font())
    }

    /// Gets the pak-unique `BitmapId` corresponding to the given key, if one exsits.
    fn bitmap_id(&self, key: impl AsRef<str>) -> Option<BitmapId> {
        self.data
            .ids
            .get(key.as_ref())
            .and_then(|id| id.as_bitmap())
    }

    /// Gets the pak-unique `BlobId` corresponding to the given key, if one exsits.
    fn blob_id(&self, key: impl AsRef<str>) -> Option<BlobId> {
        self.data.ids.get(key.as_ref()).and_then(|id| id.as_blob())
    }

    /// Gets the pak-unique `MaterialId` corresponding to the given key, if one exsits.
    fn material_id(&self, key: impl AsRef<str>) -> Option<MaterialId> {
        self.data
            .ids
            .get(key.as_ref())
            .and_then(|id| id.as_material())
    }

    /// Gets the pak-unique `ModelId` corresponding to the given key, if one exsits.
    fn model_id(&self, key: impl AsRef<str>) -> Option<ModelId> {
        self.data.ids.get(key.as_ref()).and_then(|id| id.as_model())
    }

    /// Gets the pak-unique `SceneId` corresponding to the given key, if one exsits.
    fn scene_id(&mut self, key: impl AsRef<str>) -> Option<SceneId> {
        self.data.ids.get(key.as_ref()).and_then(|id| id.as_scene())
    }

    /// Gets the corresponding animation for the given ID.
    fn read_animation(&mut self, id: AnimationId) -> Result<AnimationBuf, Error> {
        let (pos, len) = self.data.anims[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }

    /// Reads the corresponding bitmap for the given ID.
    fn read_bitmap_font(&mut self, id: BitmapFontId) -> Result<BitmapFontBuf, Error> {
        let (pos, len) = self.data.bitmap_fonts[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }

    /// Reads the corresponding bitmap for the given ID.
    fn read_bitmap(&mut self, id: BitmapId) -> Result<BitmapBuf, Error> {
        let (pos, len) = self.data.bitmaps[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }

    /// Gets the corresponding blob for the given ID.
    fn read_blob(&mut self, id: BlobId) -> Result<Vec<u8>, Error> {
        let (pos, len) = self.data.blobs[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }

    /// Gets the material for the given ID.
    fn read_material(&self, id: MaterialId) -> Option<MaterialInfo> {
        self.data.materials.get(id.0).copied()
    }

    /// Gets the corresponding animation for the given ID.
    fn read_model(&mut self, id: ModelId) -> Result<ModelBuf, Error> {
        let (pos, len) = self.data.models[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }

    /// Gets the corresponding animation for the given ID.
    fn read_scene(&mut self, id: SceneId) -> Result<SceneBuf, Error> {
        let (pos, len) = self.data.scenes[id.0]
            .pos_len()
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        self.deserialize(pos, len)
    }
}

#[derive(Debug)]
struct PakFile {
    buf: BufReader<File>,
    path: PathBuf,
}

impl From<&'static [u8]> for PakBuf {
    fn from(data: &'static [u8]) -> Self {
        // This is infalliable for the given input so unwrap is aok
        Self::from_stream(Cursor::new(data)).unwrap()
    }
}

pub trait Stream: Debug + Read + Seek + Send {
    fn open(&self) -> Result<Box<dyn Stream>, Error>;
}

impl Stream for PakFile {
    fn open(&self) -> Result<Box<dyn Stream>, Error> {
        let file = File::open(&self.path)?;
        let buf = BufReader::new(file);

        Ok(Box::new(PakFile {
            buf,
            path: self.path.clone(),
        }))
    }
}

impl Read for PakFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.buf.read(buf)
    }
}

impl Seek for PakFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.buf.seek(pos)
    }
}

impl Stream for Cursor<&'static [u8]> {
    fn open(&self) -> Result<Box<dyn Stream>, Error> {
        Ok(Box::new(Cursor::new(*self.get_ref())))
    }
}
