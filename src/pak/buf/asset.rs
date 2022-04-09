use {
    super::{
        anim::Animation,
        bitmap::Bitmap,
        blob::Blob,
        content::Content,
        material::{Material, MaterialParams},
        model::Model,
        scene::Scene,
    },
    serde::Deserialize,
    std::{
        fs::read_to_string,
        io::{Error, ErrorKind},
        path::Path,
    },
};

/// A collection type containing all supported asset file types.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum Asset {
    /// `.glb` or `.gltf` model animations.
    Animation(Animation),
    /// `.jpeg` and other regular images.
    Bitmap(Bitmap),
    /// `.fnt` bitmapped fonts.
    BitmapFont(Blob),
    /// Raw byte blobs.
    Blob(Blob),
    /// Solid color.
    ColorRgb([u8; 3]),
    /// Solid color with alpha channel.
    ColorRgba([u8; 4]),
    /// Top-level content files which simply group other asset files for ease of use.
    Content(Content),
    /// Used for 3D model rendering.
    Material(Material),
    /// Used to cache the params texture during material baking.
    MaterialParams(MaterialParams),
    /// `.glb` or `.gltf` 3D models.
    Model(Model),
    /// Describes position/orientation/scale and tagged data specific to each program.
    ///
    /// You are expected to write some manner of and export tool in order to create this file type
    /// using an external editor.
    Scene(Scene),
}

impl Asset {
    /// Reads an asset file from disk.
    #[allow(unused)]
    pub fn read(filename: impl AsRef<Path>) -> Result<Self, Error> {
        let str = read_to_string(&filename)?;
        let val: Schema = toml::from_str(&str)?;
        let res = if let Some(val) = val.anim {
            Self::Animation(val)
        } else if let Some(val) = val.bitmap {
            Self::Bitmap(val)
        } else if let Some(val) = val.bitmap_font {
            Self::BitmapFont(val)
        } else if let Some(val) = val.content {
            Self::Content(val)
        } else if let Some(val) = val.material {
            Self::Material(val)
        } else if let Some(val) = val.model {
            Self::Model(val)
        } else if let Some(val) = val.scene {
            Self::Scene(val)
        } else {
            return Err(Error::from(ErrorKind::InvalidData));
        };

        Ok(res)
    }

    /// Attempts to extract a `Bitmap` asset from this collection type.
    #[allow(unused)]
    pub fn into_bitmap(self) -> Option<Bitmap> {
        match self {
            Self::Bitmap(bitmap) => Some(bitmap),
            _ => None,
        }
    }

    /// Attempts to extract a `Content` asset from this collection type.
    #[allow(unused)]
    pub fn into_content(self) -> Option<Content> {
        match self {
            Self::Content(content) => Some(content),
            _ => None,
        }
    }

    /// Attempts to extract a `Material` asset from this collection type.
    #[allow(unused)]
    pub fn into_material(self) -> Option<Material> {
        match self {
            Self::Material(material) => Some(material),
            _ => None,
        }
    }

    /// Attempts to extract a `Model` asset from this collection type.
    #[allow(unused)]
    pub fn into_model(self) -> Option<Model> {
        match self {
            Self::Model(model) => Some(model),
            _ => None,
        }
    }
}

impl From<Bitmap> for Asset {
    fn from(val: Bitmap) -> Self {
        Self::Bitmap(val)
    }
}

impl From<[u8; 3]> for Asset {
    fn from(val: [u8; 3]) -> Self {
        Self::ColorRgb(val)
    }
}

impl From<[u8; 4]> for Asset {
    fn from(val: [u8; 4]) -> Self {
        Self::ColorRgba(val)
    }
}

impl From<Model> for Asset {
    fn from(val: Model) -> Self {
        Self::Model(val)
    }
}

impl From<Material> for Asset {
    fn from(val: Material) -> Self {
        Self::Material(val)
    }
}

impl From<Scene> for Asset {
    fn from(val: Scene) -> Self {
        Self::Scene(val)
    }
}

#[derive(Deserialize)]
struct Schema {
    #[serde(rename = "animation")]
    #[allow(unused)]
    anim: Option<Animation>,

    #[allow(unused)]
    bitmap: Option<Bitmap>,

    #[serde(rename = "bitmap-font")]
    #[allow(unused)]
    bitmap_font: Option<Blob>,

    #[allow(unused)]
    content: Option<Content>,
    #[allow(unused)]
    material: Option<Material>,
    #[allow(unused)]
    model: Option<Model>,
    #[allow(unused)]
    scene: Option<Scene>,
}
