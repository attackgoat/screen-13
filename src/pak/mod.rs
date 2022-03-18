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
    model::{IndexType, Mesh, ModelBuf},
    scene::{SceneBuf, SceneBufRef, SceneRefData},
};

use {
    paste::paste,
    serde::{Deserialize, Serialize},
    std::io::{Error, ErrorKind},
};

macro_rules! id_struct {
    ($name: ident) => {
        paste! {
            #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Ord,
                Serialize)]
            pub struct [<$name Id>](usize);
        }
    };
}

id_struct!(Animation);
id_struct!(Bitmap);
id_struct!(BitmapFont);
id_struct!(Blob);
id_struct!(Material);
id_struct!(Model);
id_struct!(Scene);

/// Holds bitmap handles to match what was setup in the asset `.toml` file.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MaterialInfo {
    /// Three or four channel base color, aka albedo or diffuse, of the material.
    pub color: BitmapId,

    /// A standard three channel emissive color map.
    pub emissive: Option<BitmapId>,

    /// A standard three channel normal map.
    pub normal: BitmapId,

    /// A two channel bitmap of the metalness (red) and roughness (green) PBR parameters.
    ///
    /// Optionally has a third channel (blue) for displacement.
    pub params: BitmapId,
}

pub trait Pak {
    // --- "Get by id" functions

    /// Gets the pak-unique `AnimationId` corresponding to the given key, if one exsits.
    fn animation_id(&self, key: impl AsRef<str>) -> Option<AnimationId>;

    /// Gets the pak-unique `BitmapFontId` corresponding to the given key, if one exsits.
    fn bitmap_font_id(&self, key: impl AsRef<str>) -> Option<BitmapFontId>;

    /// Gets the pak-unique `BitmapId` corresponding to the given key, if one exsits.
    fn bitmap_id(&self, key: impl AsRef<str>) -> Option<BitmapId>;

    /// Gets the pak-unique `BlobId` corresponding to the given key, if one exsits.
    fn blob_id(&self, key: impl AsRef<str>) -> Option<BlobId>;

    /// Gets the pak-unique `MaterialId` corresponding to the given key, if one exsits.
    fn material_id(&self, key: impl AsRef<str>) -> Option<MaterialId>;

    /// Gets the pak-unique `ModelId` corresponding to the given key, if one exsits.
    fn model_id(&self, key: impl AsRef<str>) -> Option<ModelId>;

    /// Gets the pak-unique `SceneId` corresponding to the given key, if one exsits.
    fn scene_id(&mut self, key: impl AsRef<str>) -> Option<SceneId>;

    // --- "Read" functions

    /// Gets the corresponding animation for the given ID.
    fn read_animation(&mut self, id: AnimationId) -> Result<AnimationBuf, Error>;

    /// Reads the corresponding bitmap for the given ID.
    fn read_bitmap_font(&mut self, id: BitmapFontId) -> Result<BitmapFontBuf, Error>;

    /// Reads the corresponding bitmap for the given ID.
    fn read_bitmap(&mut self, id: BitmapId) -> Result<BitmapBuf, Error>;

    /// Gets the corresponding blob for the given ID.
    fn read_blob(&mut self, id: BlobId) -> Result<Vec<u8>, Error>;

    /// Gets the material for the given handle, if one exsits.
    fn read_material(&self, id: MaterialId) -> Option<MaterialInfo>;

    /// Gets the corresponding animation for the given ID.
    fn read_model(&mut self, id: ModelId) -> Result<ModelBuf, Error>;

    /// Gets the corresponding animation for the given ID.
    fn read_scene(&mut self, id: SceneId) -> Result<SceneBuf, Error>;

    // --- Convenience functions

    /// Gets the material corresponding to the given key, if one exsits.
    fn read_material_key(&self, key: impl AsRef<str>) -> Option<MaterialInfo> {
        if let Some(id) = self.material_id(key) {
            self.read_material(id)
        } else {
            None
        }
    }

    fn read_animation_key(&mut self, key: impl AsRef<str>) -> Result<AnimationBuf, Error> {
        if let Some(h) = self.animation_id(key) {
            self.read_animation(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_bitmap_font_key(&mut self, key: impl AsRef<str>) -> Result<BitmapFontBuf, Error> {
        if let Some(h) = self.bitmap_font_id(key) {
            self.read_bitmap_font(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_bitmap_key(&mut self, key: impl AsRef<str>) -> Result<BitmapBuf, Error> {
        if let Some(h) = self.bitmap_id(key) {
            self.read_bitmap(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_blob_key(&mut self, key: impl AsRef<str>) -> Result<Vec<u8>, Error> {
        if let Some(h) = self.blob_id(key) {
            self.read_blob(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_model_key(&mut self, key: impl AsRef<str>) -> Result<ModelBuf, Error> {
        if let Some(h) = self.model_id(key) {
            self.read_model(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }

    fn read_scene_key(&mut self, key: impl AsRef<str>) -> Result<SceneBuf, Error> {
        if let Some(h) = self.scene_id(key) {
            self.read_scene(h)
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }
}
