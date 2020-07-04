use {
    crate::math::{vec3, Vec3},
    serde::{Deserialize, Serialize},
    serde_json::{
        map::Map,
        {from_reader, Value},
    },
    std::{
        collections::HashMap,
        fs::File,
        path::{Path, PathBuf},
    },
};

fn parse_hashmap(value: &Map<String, Value>) -> HashMap<String, String> {
    let mut result = HashMap::default();
    for entry in value {
        let key = entry.0.clone();
        let value = entry.1.as_str().unwrap().to_string();
        result.insert(key, value);
    }

    result
}

pub fn parse_vector3(value: &str) -> [f32; 3] {
    let mut parts = value.split(',');
    let x = parts.next().unwrap().parse().unwrap();
    let y = parts.next().unwrap().parse().unwrap();
    let z = parts.next().unwrap().parse().unwrap();

    [x, y, z]
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Asset {
    Bitmap(BitmapAsset),
    FontBitmap(FontBitmapAsset),
    Language(LanguageAsset),
    Mesh(MeshAsset),
    Scene(SceneAsset),
}

impl Asset {
    pub fn read<P: AsRef<Path>>(filename: P) -> Self {
        let file = File::open(&filename).unwrap();
        let value = from_reader(file).unwrap();

        Self::parse_json(&value)
    }

    pub fn into_mesh(self) -> Option<MeshAsset> {
        match self {
            Self::Mesh(res) => Some(res),
            _ => None,
        }
    }

    fn parse_json(value: &Value) -> Self {
        match value["type"].as_str().unwrap() {
            "BITMAP" => Asset::Bitmap(BitmapAsset::parse_json(value)),
            "FONT_BITMAP" => Asset::FontBitmap(FontBitmapAsset::parse_json(value)),
            "LOCALIZATION" => Asset::Language(LanguageAsset::parse_json(value)),
            "MESH" => Asset::Mesh(MeshAsset::parse_json(value)),
            "SCENE" => Asset::Scene(SceneAsset::parse_json(value)),
            _ => unimplemented!(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct BitmapAsset {
    bitmap: PathBuf,
    force_opaque: bool,
}

impl BitmapAsset {
    fn parse_json(value: &Value) -> Self {
        Self {
            bitmap: PathBuf::from(value["bitmap"].as_str().unwrap()),
            force_opaque: value["forceOpaque"].as_bool().unwrap(),
        }
    }

    pub fn bitmap(&self) -> &Path {
        self.bitmap.as_path()
    }

    pub fn force_opaque(&self) -> bool {
        self.force_opaque
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct FontBitmapAsset {
    bitmap: PathBuf,
}

impl FontBitmapAsset {
    fn parse_json(value: &Value) -> Self {
        Self {
            bitmap: PathBuf::from(value["bitmap"].as_str().unwrap()),
        }
    }

    pub fn bitmap(&self) -> &Path {
        self.bitmap.as_path()
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct LanguageAsset {
    locale: String,
    text: HashMap<String, String>,
}

impl LanguageAsset {
    fn parse_json(value: &Value) -> Self {
        Self {
            locale: value["locale"]
                .as_str()
                .expect("unspecified locale")
                .to_string(),
            text: parse_hashmap(value["text"].as_object().expect("unspecified text")),
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn text(&self) -> &HashMap<String, String> {
        &self.text
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct MeshAsset {
    pub bitmaps: Vec<PathBuf>,
    pub scale: [f32; 3],
    pub src: PathBuf,
    pub translation: [f32; 3],
}

impl MeshAsset {
    fn parse_json(value: &Value) -> Self {
        let bitmaps = value["bitmaps"].as_array().expect("unspecified bitmaps");

        Self {
            bitmaps: bitmaps
                .iter()
                .map(|bitmap| PathBuf::from(bitmap.as_str().expect("unspecified bitmap")))
                .collect(),
            scale: parse_vector3(value["scale"].as_str().unwrap_or("1,1,1")),
            src: PathBuf::from(value["src"].as_str().expect("unspecified src")),
            translation: parse_vector3(value["translation"].as_str().unwrap_or("0,0,0")),
        }
    }

    pub fn bitmaps(&self) -> &[PathBuf] {
        &self.bitmaps
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }

    pub fn scale(&self) -> Vec3 {
        vec3(self.scale[0], self.scale[1], self.scale[2])
    }

    pub fn translation(&self) -> Vec3 {
        vec3(
            self.translation[0],
            self.translation[1],
            self.translation[2],
        )
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SceneAsset {
    items: Vec<SceneItemAsset>,
}

impl SceneAsset {
    fn parse_json(value: &Value) -> Self {
        let mut items = vec![];
        for item in value["items"].as_array().unwrap() {
            items.push(SceneItemAsset::parse_json(item));
        }

        Self { items }
    }

    pub fn items(&self) -> &[SceneItemAsset] {
        &self.items
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SceneItemAsset {
    pub id: String,
    pub key: String,
    position: [f32; 3],
    roll_pitch_yaw: [f32; 3],
    tags: Vec<String>,
}

impl SceneItemAsset {
    fn parse_json(value: &Value) -> Self {
        let mut tags = vec![];
        for tag in value["tag"].as_str().unwrap_or("").to_owned().split(' ') {
            tags.push(tag.to_owned());
        }

        Self {
            id: value["id"].as_str().unwrap_or("").to_owned(),
            key: value["key"].as_str().unwrap_or("").to_owned(),
            position: parse_vector3(value["pos"].as_str().unwrap_or("0,0,0")),
            roll_pitch_yaw: parse_vector3(value["rpy"].as_str().unwrap_or("0,0,0")),
            tags,
        }
    }

    pub fn position(&self) -> Vec3 {
        vec3(self.position[0], self.position[1], self.position[2])
    }

    pub fn roll_pitch_yaw(&self) -> Vec3 {
        vec3(
            self.roll_pitch_yaw[0],
            self.roll_pitch_yaw[1],
            self.roll_pitch_yaw[2],
        )
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }
}
