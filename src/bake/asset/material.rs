use {
    super::{parse_hex_color, parse_hex_scalar, Bitmap, Canonicalize},
    crate::color::WHITE,
    serde::{
        de::{
            self,
            value::{MapAccessDeserializer, SeqAccessDeserializer},
            MapAccess, SeqAccess, Visitor,
        },
        Deserialize, Deserializer,
    },
    std::{
        fmt,
        num::FpCategory,
        path::{Path, PathBuf},
        u8,
    },
};

/// A reference to a `Bitmap` asset, `Bitmap` asset file, three or four channel image source file,
/// or single four channel color.
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum ColorRef {
    /// A `Bitmap` asset specified inline.
    Asset(Bitmap),

    /// A `Bitmap` asset file or image source file.
    Path(PathBuf),

    /// A single four channel color.
    Value([u8; 4]),
}

impl ColorRef {
    /// Deserialize from any of:
    ///
    /// val of [0.666, 0.733, 0.8, 1.0]:
    /// .. = "#abc"
    /// .. = "#abcf"
    /// .. = "#aabbcc"
    /// .. = "#aabbccff"
    /// .. = [0.666, 0.733, 0.8, 1.0]
    ///
    /// src of file.png:
    /// .. = "file.png"
    ///
    /// src of file.toml which must be a `Bitmap` asset:
    /// .. = "file.toml"
    ///
    /// src of a `Bitmap` asset:
    /// .. = { src = "file.png", format = "Rgb" }
    fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorRefVisitor;

        impl<'de> Visitor<'de> for ColorRefVisitor {
            type Value = ColorRef;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("hex string, path string, bitmap asset, or seqeunce")
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(ColorRef::Asset(asset))
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut val: Vec<f32> = Deserialize::deserialize(SeqAccessDeserializer::new(seq))?;
                for val in &val {
                    match val.classify() {
                        FpCategory::Zero | FpCategory::Normal if *val <= 1.0 => (),
                        _ => panic!("Unexpected color value"),
                    }
                }

                match val.len() {
                    3 => val.push(1.0),
                    4 => (),
                    _ => panic!("Unexpected color length"),
                }

                Ok(ColorRef::Value([
                    (val[0] * u8::MAX as f32) as u8,
                    (val[1] * u8::MAX as f32) as u8,
                    (val[2] * u8::MAX as f32) as u8,
                    (val[3] * u8::MAX as f32) as u8,
                ]))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if str.starts_with('#') {
                    if let Some(val) = parse_hex_color(str) {
                        return Ok(ColorRef::Value(val));
                    }
                }

                Ok(ColorRef::Path(PathBuf::from(str)))
            }
        }

        deserializer.deserialize_any(ColorRefVisitor)
    }
}

impl Canonicalize for ColorRef {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
            _ => (),
        }
    }
}

impl Default for ColorRef {
    fn default() -> Self {
        Self::Value(WHITE.into())
    }
}

/// Holds a description of data used for model rendering.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Material {
    #[serde(default, deserialize_with = "ColorRef::de")]
    color: ColorRef,

    double_sided: Option<bool>,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    metal: Option<ScalarRef>,

    #[serde(default, deserialize_with = "NormalRef::de")]
    normal: Option<NormalRef>,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    rough: Option<ScalarRef>,
}

impl Material {
    pub(crate) fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            color: ColorRef::Path(src.as_ref().to_owned()),
            ..Default::default()
        }
    }

    /// A `Bitmap` asset, `Bitmap` asset file, three or four channel image source file, or single
    /// four channel color.
    pub fn color(&self) -> &ColorRef {
        &self.color
    }

    /// Whether or not the model will be rendered with back faces also enabled.
    pub fn double_sided(&self) -> bool {
        self.double_sided.unwrap_or_default()
    }

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    pub fn metal(&self) -> Option<&ScalarRef> {
        self.metal.as_ref()
    }

    /// A bitmap asset, bitmap asset file, or a three channel image.
    pub fn normal(&self) -> Option<&NormalRef> {
        self.normal.as_ref()
    }

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    pub fn rough(&self) -> Option<&ScalarRef> {
        self.rough.as_ref()
    }
}

impl Canonicalize for Material {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.color.canonicalize(&project_dir, &src_dir);
        self.metal
            .as_mut()
            .map(|metal| metal.canonicalize(&project_dir, &src_dir));
        self.normal
            .as_mut()
            .map(|normal| normal.canonicalize(&project_dir, &src_dir));
        self.rough
            .as_mut()
            .map(|rough| rough.canonicalize(&project_dir, &src_dir));
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: ColorRef::Value(WHITE.into()),
            double_sided: None,
            metal: None,
            normal: None,
            rough: None,
        }
    }
}

/// A reference to a bitmap asset, bitmap asset file, or three channel image source file.
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum NormalRef {
    /// A `Bitmap` asset specified inline.
    Asset(Bitmap),

    /// A `Bitmap` asset file or three channel image source file.
    Path(PathBuf),
}

impl NormalRef {
    /// Deserialize from any of absent or:
    ///
    /// src of file.png:
    /// .. = "file.png"
    ///
    /// src of file.toml which must be a Bitmap asset:
    /// .. = "file.toml"
    ///
    /// src of a Bitmap asset:
    /// .. = { src = "file.png", format = "Rgb" }
    fn de<'de, D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NormalRefVisitor;

        impl<'de> Visitor<'de> for NormalRefVisitor {
            type Value = Option<NormalRef>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("path string or bitmap asset")
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(NormalRef::Asset(asset)))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Some(NormalRef::Path(PathBuf::from(str))))
            }
        }

        deserializer.deserialize_any(NormalRefVisitor)
    }
}

impl Canonicalize for NormalRef {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
        }
    }
}

/// Reference to a `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a
/// single value.
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum ScalarRef {
    /// A `Bitmap` asset specified inline.
    Asset(Bitmap),

    /// A `Bitmap` asset file or single channel image source file.
    Path(PathBuf),

    /// A single value.
    Value(u8),
}

impl ScalarRef {
    /// Deserialize from any of absent or:
    ///
    /// val of 1.0:
    /// .. = "#f"
    /// .. = "#ff"
    /// .. = 1.0
    ///
    /// src of file.png:
    /// .. = "file.png"
    ///
    /// src of file.toml which must be a Bitmap asset:
    /// .. = "file.toml"
    ///
    /// src of a Bitmap asset:
    /// .. = { src = "file.png", format = "R" }
    fn de<'de, D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ScalarRefVisitor;

        impl<'de> Visitor<'de> for ScalarRefVisitor {
            type Value = Option<ScalarRef>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("hex string, path string, bitmap asset, or floating point value")
            }

            fn visit_f64<E>(self, val: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let val = val as f32;
                match val.classify() {
                    FpCategory::Zero | FpCategory::Normal if val <= 1.0 => (),
                    _ => panic!("Unexpected scalar value"),
                }

                Ok(Some(ScalarRef::Value((val * u8::MAX as f32) as _)))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(ScalarRef::Asset(asset)))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if str.starts_with('#') {
                    if let Some(val) = parse_hex_scalar(str) {
                        return Ok(Some(ScalarRef::Value(val)));
                    }
                }

                Ok(Some(ScalarRef::Path(PathBuf::from(str))))
            }
        }

        deserializer.deserialize_any(ScalarRefVisitor)
    }
}

impl Canonicalize for ScalarRef {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
            _ => (),
        }
    }
}
