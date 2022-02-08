pub mod buf;
pub mod compression;

mod anim;
mod bitmap;
mod bitmap_font;
mod model;
mod scene;

pub use self::{
    anim::{AnimationBuf, Channel},
    bitmap::{BitmapBuf, BitmapColor, BitmapFormat},
    bitmap_font::BitmapFontBuf,
    model::ModelBuf,
    scene::{Instance, SceneBuf},
};

use {
    paste::paste,
    serde::{Deserialize, Serialize},
    std::io::{Error, ErrorKind},
};

macro_rules! handle_struct {
    ($name: ident) => {
        paste! {
            #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Ord,
                Serialize)]
            pub struct [<$name Handle>](usize);
        }
    };
}

handle_struct!(Animation);
handle_struct!(Bitmap);
handle_struct!(BitmapFont);
handle_struct!(Blob);
handle_struct!(Material);
handle_struct!(Model);
handle_struct!(Scene);

/// Holds bitmap handles to match what was setup in the asset `.toml` file.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MaterialInfo {
    /// Three or four channel base color, aka albedo or diffuse, of the material.
    pub color: BitmapHandle,

    /// A standard three channel normal map.
    pub normal: BitmapHandle,

    /// A two channel bitmap of the metalness (red) and roughness (green) PBR parameters.
    ///
    /// Optionally has a third channel (blue) for displacement.
    pub params: BitmapHandle,
}

pub trait Pak {
    // --- Material functions (the desc is Copy and expected to be in memory)

    /// Gets the pak-unique `MaterialHandle` corresponding to the given key, if one exsits.
    fn material_handle(&self, key: impl AsRef<str>) -> Option<MaterialHandle>;

    /// Gets the material for the given handle.
    fn material(&self, handle: MaterialHandle) -> Option<MaterialInfo>;

    // --- "Get by handle" functions

    /// Gets the pak-unique `AnimationHandle` corresponding to the given key, if one exsits.
    fn animation_handle(&self, key: impl AsRef<str>) -> Option<AnimationHandle>;

    /// Gets the pak-unique `BitmapHandle` corresponding to the given key, if one exsits.
    fn bitmap_font_handle(&self, key: impl AsRef<str>) -> Option<BitmapFontHandle>;

    /// Gets the pak-unique `BitmapHandle` corresponding to the given key, if one exsits.
    fn bitmap_handle(&self, key: impl AsRef<str>) -> Option<BitmapHandle>;

    /// Gets the pak-unique `BlobHandle` corresponding to the given key, if one exsits.
    fn blob_handle(&self, key: impl AsRef<str>) -> Option<BlobHandle>;

    /// Gets the pak-unique `ModelHandle` corresponding to the given key, if one exsits.
    fn model_handle(&self, key: impl AsRef<str>) -> Option<ModelHandle>;

    /// Gets the pak-unique `SceneHandle` corresponding to the given key, if one exsits.
    fn scene_handle(&mut self, key: impl AsRef<str>) -> Option<SceneHandle>;

    // --- "Read" functions

    /// Gets the corresponding animation for the given handle.
    fn read_animation(&mut self, handle: AnimationHandle) -> Result<AnimationBuf, Error>;

    /// Reads the corresponding bitmap for the given handle.
    fn read_bitmap_font(&mut self, handle: BitmapFontHandle) -> Result<BitmapFontBuf, Error>;

    /// Reads the corresponding bitmap for the given handle.
    fn read_bitmap(&mut self, handle: BitmapHandle) -> Result<BitmapBuf, Error>;

    /// Gets the corresponding blob for the given handle.
    fn read_blob(&mut self, handle: BlobHandle) -> Result<Vec<u8>, Error>;

    /// Gets the corresponding animation for the given handle.
    fn read_model(&mut self, handle: ModelHandle) -> Result<ModelBuf, Error>;

    /// Gets the corresponding animation for the given handle.
    fn read_scene(&mut self, handle: SceneHandle) -> Result<SceneBuf, Error>;

    // --- Convenience functions

    fn read_animation_key(&mut self, key: impl AsRef<str>) -> Result<AnimationBuf, Error> {
        if let Some(h) = self.animation_handle(key) {
            self.read_animation(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_bitmap_font_key(&mut self, key: impl AsRef<str>) -> Result<BitmapFontBuf, Error> {
        if let Some(h) = self.bitmap_font_handle(key) {
            self.read_bitmap_font(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_bitmap_key(&mut self, key: impl AsRef<str>) -> Result<BitmapBuf, Error> {
        if let Some(h) = self.bitmap_handle(key) {
            self.read_bitmap(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_blob_key(&mut self, key: impl AsRef<str>) -> Result<Vec<u8>, Error> {
        if let Some(h) = self.blob_handle(key) {
            self.read_blob(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_model_key(&mut self, key: impl AsRef<str>) -> Result<ModelBuf, Error> {
        if let Some(h) = self.model_handle(key) {
            self.read_model(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_scene_key(&mut self, key: impl AsRef<str>) -> Result<SceneBuf, Error> {
        if let Some(h) = self.scene_handle(key) {
            self.read_scene(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }
}
