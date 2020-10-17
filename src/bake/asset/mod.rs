mod bitmap;
mod font_bitmap;
mod model;
mod scene;

pub use self::{bitmap::Bitmap, font_bitmap::FontBitmap, model::Model, scene::Scene};

use {
    serde::{Deserialize, Serialize},
    std::{fs::read_to_string, path::Path},
    toml::from_str,
};

#[derive(Clone, Deserialize, Serialize)]
pub enum Asset {
    // Atlas(AtlasAsset),
    Bitmap(Bitmap),
    FontBitmap(FontBitmap),
    // Language(LanguageAsset),
    Model(Model),
    Scene(Scene),
}

impl Asset {
    pub fn read<P: AsRef<Path>>(filename: P) -> Self {
        let val: Schema = from_str(&read_to_string(&filename).expect(&format!(
            "Could not parse asset file {}",
            filename.as_ref().display()
        )))
        .expect(&format!(
            "Could not parse asset file {}",
            filename.as_ref().display()
        ));

        if let Some(val) = val.bitmap {
            Self::Bitmap(val)
        } else if let Some(val) = val.font_bitmap {
            Self::FontBitmap(val)
        } else if let Some(val) = val.model {
            Self::Model(val)
        } else if let Some(val) = val.scene {
            Self::Scene(val)
        } else {
            unimplemented!();
        }
    }

    pub fn into_model(self) -> Option<Model> {
        match self {
            Self::Model(res) => Some(res),
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
    bitmap: Option<Bitmap>,
    #[serde(rename = "font-bitmap")]
    font_bitmap: Option<FontBitmap>,
    model: Option<Model>,
    scene: Option<Scene>,
}
