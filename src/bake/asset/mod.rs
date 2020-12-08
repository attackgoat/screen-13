mod anim;
mod bitmap;
mod bitmap_font;
mod content;
mod material;
mod mesh;
mod model;
mod scene;

pub use self::{
    anim::Animation, bitmap::Bitmap, bitmap_font::BitmapFont, content::Content, material::Material,
    mesh::Mesh, model::Model, scene::Scene,
};

use {
    serde::Deserialize,
    std::{fs::read_to_string, path::Path},
    toml::from_str,
};

#[derive(Clone, Deserialize)]
pub enum Asset {
    Animation(Animation),
    // Atlas(AtlasAsset),
    Bitmap(Bitmap),
    BitmapFont(BitmapFont),
    Content(Content),
    // Language(LanguageAsset),
    Material(Material),
    Model(Model),
    Scene(Scene),
}

impl Asset {
    pub fn read<P: AsRef<Path>>(filename: P) -> Self {
        let val: Schema = from_str(&read_to_string(&filename).unwrap_or_else(|_| {
            panic!("Could not parse asset file {}", filename.as_ref().display())
        }))
        .unwrap_or_else(|_| panic!("Could not parse asset file {}", filename.as_ref().display()));

        if let Some(val) = val.anim {
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
            panic!("Could not parse asset file {}", filename.as_ref().display());
        }
    }

    pub fn into_bitmap(self) -> Option<Bitmap> {
        match self {
            Self::Bitmap(bitmap) => Some(bitmap),
            _ => None,
        }
    }

    pub fn into_content(self) -> Option<Content> {
        match self {
            Self::Content(content) => Some(content),
            _ => None,
        }
    }

    pub fn into_material(self) -> Option<Material> {
        match self {
            Self::Material(material) => Some(material),
            _ => None,
        }
    }

    pub fn into_model(self) -> Option<Model> {
        match self {
            Self::Model(model) => Some(model),
            _ => None,
        }
    }
}

// #[derive(Clone, Deserialize, Serialize)]
// pub struct AtlasAsset {
//     tiles: Vec<AtlasTile>,
// }

// #[derive(Clone, Deserialize, Serialize)]
// pub struct AtlasTile {
//     bitmap: PathBuf,
//     src: Rect,
//     dst: Coord,
// }

// #[derive(Clone, Deserialize, Serialize)]
// pub struct LanguageAsset {
//     locale: String,
//     text: HashMap<String, String>,
// }

// impl LanguageAsset {
//     fn parse_json(value: &Value) -> Self {
//         Self {
//             locale: value["locale"]
//                 .as_str()
//                 .expect("unspecified locale")
//                 .to_string(),
//             text: parse_hashmap(value["text"].as_object().expect("unspecified text")),
//         }
//     }

//     pub fn locale(&self) -> &str {
//         &self.locale
//     }

//     pub fn text(&self) -> &HashMap<String, String> {
//         &self.text
//     }
// }

#[derive(Deserialize)]
struct Schema {
    #[serde(rename = "animation")]
    anim: Option<Animation>,

    bitmap: Option<Bitmap>,

    #[serde(rename = "bitmap-font")]
    bitmap_font: Option<BitmapFont>,

    content: Option<Content>,
    material: Option<Material>,
    model: Option<Model>,
    scene: Option<Scene>,
}
