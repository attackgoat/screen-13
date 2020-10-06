use {
    crate::math::Vec3,
    serde::{Deserialize, Serialize},
    std::{
        fs::read_to_string,
        path::{Path, PathBuf},
    },
    toml::from_str,
};

// fn parse_hashmap(value: &Map<String, Value>) -> HashMap<String, String> {
//     let mut result = HashMap::default();
//     for entry in value {
//         let key = entry.0.clone();
//         let value = entry.1.as_str().unwrap().to_string();
//         result.insert(key, value);
//     }

//     result
// }

pub fn parse_vector3(value: &str) -> [f32; 3] {
    let mut parts = value.split(',');
    let x = parts.next().unwrap().parse().unwrap();
    let y = parts.next().unwrap().parse().unwrap();
    let z = parts.next().unwrap().parse().unwrap();

    [x, y, z]
}

#[derive(Clone, Serialize)]
pub enum Asset {
    // Atlas(AtlasAsset),
    Bitmap(BitmapAsset),
    FontBitmap(FontBitmapAsset),
    // Language(LanguageAsset),
    Model(ModelAsset),
    // Scene(SceneAsset),
}

impl Asset {
    pub fn read<P: AsRef<Path>>(filename: P) -> Self {
        let val: Schema = from_str(&read_to_string(&filename).expect("Could not read asset file"))
            .expect("Could not parse asset file");

        if let Some(val) = val.bitmap {
            Self::Bitmap(val)
        } else if let Some(val) = val.font_bitmap {
            Self::FontBitmap(val)
        } else if let Some(val) = val.model {
            Self::Model(val)
        } else {
            unimplemented!();
        }
    }

    pub fn into_model(self) -> Option<ModelAsset> {
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

#[derive(Clone, Deserialize, Serialize)]
pub struct BitmapAsset {
    bitmap: PathBuf,
    force_opaque: bool,
}

impl BitmapAsset {
    pub fn bitmap(&self) -> &Path {
        self.bitmap.as_path()
    }

    pub fn force_opaque(&self) -> bool {
        self.force_opaque
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct FontBitmapAsset {
    src: PathBuf,
}

impl FontBitmapAsset {
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

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

#[derive(Clone, Deserialize, Serialize)]
pub struct ModelAsset {
    pub bitmaps: Vec<PathBuf>,
    pub scale: Vec3,
    pub src: PathBuf,
    pub offset: Vec3,
}

impl ModelAsset {
    pub fn bitmaps(&self) -> &[PathBuf] {
        &self.bitmaps
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

// #[derive(Clone, Deserialize, Serialize)]
// pub struct SceneAsset {
//     items: Vec<SceneItemAsset>,
// }

// impl SceneAsset {
//     fn parse_json(value: &Value) -> Self {
//         let mut items = vec![];
//         for item in value["items"].as_array().unwrap() {
//             items.push(SceneItemAsset::parse_json(item));
//         }

//         Self { items }
//     }

//     pub fn items(&self) -> &[SceneItemAsset] {
//         &self.items
//     }
// }

// #[derive(Clone, Deserialize, Serialize)]
// pub struct SceneItemAsset {
//     pub id: String,
//     pub key: String,
//     pos: [f32; 3],
//     roll_pitch_yaw: [f32; 3],
//     tags: Vec<String>,
// }

// impl SceneItemAsset {
//     fn parse_json(value: &Value) -> Self {
//         let mut tags = vec![];
//         for tag in value["tag"].as_str().unwrap_or("").to_owned().split(' ') {
//             tags.push(tag.to_owned());
//         }

//         Self {
//             id: value["id"].as_str().unwrap_or("").to_owned(),
//             key: value["key"].as_str().unwrap_or("").to_owned(),
//             pos: parse_vector3(value["pos"].as_str().unwrap_or("0,0,0")),
//             roll_pitch_yaw: parse_vector3(value["rpy"].as_str().unwrap_or("0,0,0")),
//             tags,
//         }
//     }

//     pub fn position(&self) -> Vec3 {
//         vec3(self.pos[0], self.pos[1], self.pos[2])
//     }

//     pub fn roll_pitch_yaw(&self) -> Vec3 {
//         vec3(
//             self.roll_pitch_yaw[0],
//             self.roll_pitch_yaw[1],
//             self.roll_pitch_yaw[2],
//         )
//     }

//     pub fn tags(&self) -> &[String] {
//         &self.tags
//     }
// }

#[derive(Deserialize)]
struct Schema {
    bitmap: Option<BitmapAsset>,
    #[serde(rename = "font-bitmap")]
    font_bitmap: Option<FontBitmapAsset>,
    model: Option<ModelAsset>,
}
