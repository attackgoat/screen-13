use serde::{Deserialize, Serialize};

/// An identifier for `Animation` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AnimationId(pub(crate) u16);

/// An identifier for `Bitmap` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BitmapId(pub(crate) u16);

/// An identifier for `BitmapFont` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BitmapFontId(pub(crate) u16);

/// An identifier for byte array instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BlobId(pub(crate) u16);

/// An identifier for `Font` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FontId(pub(crate) u16);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub(crate) enum Id {
    Animation(AnimationId),
    Bitmap(BitmapId),
    BitmapFont(BitmapFontId),
    Blob(BlobId),
    Font(FontId),
    Material(MaterialId),
    Model(ModelId),
    Scene(SceneId),
    Text(TextId),
}

impl Id {
    pub fn as_animation(&self) -> Option<AnimationId> {
        match self {
            Self::Animation(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_bitmap(&self) -> Option<BitmapId> {
        match self {
            Self::Bitmap(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_bitmap_font(&self) -> Option<BitmapFontId> {
        match self {
            Self::BitmapFont(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_blob(&self) -> Option<BlobId> {
        match self {
            Self::Blob(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_font(&self) -> Option<FontId> {
        match self {
            Self::Font(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_material(&self) -> Option<MaterialId> {
        match self {
            Self::Material(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_model(&self) -> Option<ModelId> {
        match self {
            Self::Model(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_scene(&self) -> Option<SceneId> {
        match self {
            Self::Scene(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<TextId> {
        match self {
            Self::Text(id) => Some(*id),
            _ => None,
        }
    }
}

impl From<AnimationId> for Id {
    fn from(id: AnimationId) -> Self {
        Self::Animation(id)
    }
}

impl From<BitmapId> for Id {
    fn from(id: BitmapId) -> Self {
        Self::Bitmap(id)
    }
}

impl From<BitmapFontId> for Id {
    fn from(id: BitmapFontId) -> Self {
        Self::BitmapFont(id)
    }
}

impl From<BlobId> for Id {
    fn from(id: BlobId) -> Self {
        Self::Blob(id)
    }
}

impl From<FontId> for Id {
    fn from(id: FontId) -> Self {
        Self::Font(id)
    }
}

impl From<MaterialId> for Id {
    fn from(id: MaterialId) -> Self {
        Self::Material(id)
    }
}

impl From<ModelId> for Id {
    fn from(id: ModelId) -> Self {
        Self::Model(id)
    }
}

impl From<SceneId> for Id {
    fn from(id: SceneId) -> Self {
        Self::Scene(id)
    }
}

impl From<TextId> for Id {
    fn from(id: TextId) -> Self {
        Self::Text(id)
    }
}

/// An identifier for `Material` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct MaterialId(pub(crate) u16);

/// An identifier for `Model` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ModelId(pub(crate) u16);

/// An identifier for `Scene` instances which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SceneId(pub(crate) u16);

/// An identifier for text fragments which is unique within one `.pak` file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct TextId(pub(crate) u16);
