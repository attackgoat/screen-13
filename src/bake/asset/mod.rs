//! Contains deserializable types which represent all supported asset file types.

mod anim;
mod bitmap;
mod blob;
mod content;
mod material;
mod model;
mod scene;

pub use self::{
    anim::Animation,
    bitmap::Bitmap,
    blob::Blob,
    content::Content,
    material::{ColorRef, Material, NormalRef, ScalarRef},
    model::{Mesh, Model},
    scene::{AssetRef, Scene, SceneRef},
};

use {
    serde::Deserialize,
    std::{
        fs::read_to_string,
        path::{Path, PathBuf},
    },
    toml::from_str,
};

fn parse_hex_color(val: &str) -> Option<[u8; 4]> {
    let mut res = [1; 4];
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

/// A collection type containing all supported asset file types.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub enum Asset {
    /// `.glb` or `.gltf` model animations.
    Animation(Animation),
    // Atlas(AtlasAsset),
    /// `.jpeg` and other regular images.
    Bitmap(Bitmap),
    /// `.fnt` bitmapped fonts.
    BitmapFont(Blob),
    /// Solid color bitmap used internally.
    Color([u8; 4]),
    /// Top-level content files which simply group other asset files for ease of use.
    Content(Content),
    // Language(LanguageAsset),
    /// Used for model rendering.
    Material(Material),
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
    pub fn read<P: AsRef<Path>>(filename: P) -> Self {
        let val: Schema = from_str(&read_to_string(&filename).unwrap_or_else(|_| {
            panic!("Unable to parse asset file {}", filename.as_ref().display())
        }))
        .unwrap_or_else(|err| {
            error!("{}", err);
            panic!("Unable to parse asset file {}", filename.as_ref().display());
        });

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

    /// Attempts to extract a `Bitmap` asset from this collection type.
    pub fn into_bitmap(self) -> Option<Bitmap> {
        match self {
            Self::Bitmap(bitmap) => Some(bitmap),
            _ => None,
        }
    }

    /// Attempts to extract a `Content` asset from this collection type.
    pub fn into_content(self) -> Option<Content> {
        match self {
            Self::Content(content) => Some(content),
            _ => None,
        }
    }

    /// Attempts to extract a `Material` asset from this collection type.
    pub fn into_material(self) -> Option<Material> {
        match self {
            Self::Material(material) => Some(material),
            _ => None,
        }
    }

    /// Attempts to extract a `Model` asset from this collection type.
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

impl From<[u8; 4]> for Asset {
    fn from(val: [u8; 4]) -> Self {
        Self::Color(val)
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

pub(crate) trait Canonicalize {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>;

    /// Gets the fully rooted source path.
    ///
    /// If `src` is relative, then `src_dir` is used to determine the relative parent.
    /// If `src` is absolute, then `project_dir` is considered to be its root.
    fn canonicalize_project_path<P1, P2, P3>(project_dir: P1, src_dir: P2, src: P3) -> PathBuf
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
        P3: AsRef<Path>,
    {
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
    bitmap_font: Option<Blob>,

    content: Option<Content>,
    material: Option<Material>,
    model: Option<Model>,
    scene: Option<Scene>,
}
