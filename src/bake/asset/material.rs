use {
    crate::color::WHITE,
    super::Bitmap,
    serde::{
        de::{self, value::{MapAccessDeserializer, SeqAccessDeserializer}, MapAccess, SeqAccess, Visitor},
        Deserialize, Deserializer,
    },
    std::{
        fmt,
        num::FpCategory,
        path::{Path, PathBuf},
        u8,
    },
};

/// A reference to a bitmap asset, three or four channel image file, or single four channel color.
#[derive(Clone,  Eq, Hash, PartialEq)]
pub enum ColorRef {
    /// A `Bitmap` asset specified inline.
    Asset(Bitmap),

    /// A `Bitmap` asset file or image file.
    File(PathBuf),

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
    /// src of file.toml which must be a Bitmap asset:
    /// .. = "file.toml"
    ///
    /// src of a Bitmap asset:
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
                    if let Some(val) = ColorRef::parse_hex(str) {
                        return Ok(ColorRef::Value(val));
                    }
                }

                Ok(ColorRef::File(PathBuf::from(str)))
            }
        }

        deserializer.deserialize_any(ColorRefVisitor)
    }

    // TODO: Color parsing and error handling should be better and somewhere else
    fn parse_hex(val: &str) -> Option<[u8; 4]> {
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
    /// A bitmap asset, three or four channel image file, or single four channel color.
    pub fn color(&self) -> &ColorRef {
        &self.color
    }

    /// Whether or not the model will be rendered with back faces also enabled.
    pub fn double_sided(&self) -> bool {
        self.double_sided.unwrap_or_default()
    }

    /// A Bitmap asset
    pub fn metal_asset(&self) -> Option<&Bitmap> {
        self.metal.as_ref().map(|scalar| scalar.asset.as_ref()).flatten()
    }

    /// A one channel image.
    pub fn metal_src(&self) -> Option<&Path> {
        self.metal
            .as_ref()
            .map(|scalar| scalar.src.as_deref())
            .flatten()
    }

    /// A single value.
    pub fn metal_val(&self) -> Option<u8> {
        self.metal.as_ref().map(|scalar| scalar.val).flatten()
    }

    /// A Bitmap asset.
    pub fn normal_asset(&self) -> Option<&Bitmap> {
        self.normal.as_ref().map(|normal| normal.asset.as_ref()).flatten()
    }

    /// A three channel image.
    pub fn normal_src(&self) -> Option<&Path> {
        self.normal.as_ref().map(|normal| normal.src.as_deref()).flatten()
    }

    /// A bitmap asset
    pub fn rough_asset(&self) -> Option<&Bitmap> {
        self.rough.as_ref().map(|scalar| scalar.asset.as_ref()).flatten()
    }

    /// A one channel image.
    pub fn rough_src(&self) -> Option<&Path> {
        self.rough
            .as_ref()
            .map(|scalar| scalar.src.as_deref())
            .flatten()
    }

    /// A single value.
    pub fn rough_val(&self) -> Option<u8> {
        self.rough.as_ref().map(|scalar| scalar.val).flatten()
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct NormalRef {
    asset: Option<Bitmap>,
    src: Option<PathBuf>,
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

                Ok(Some(NormalRef {
                    asset: Some(asset),
                    src: None,
                }))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Some(NormalRef {
                    asset: None,
                    src: Some(PathBuf::from(str)),
                }))
            }
        }

        deserializer.deserialize_any(NormalRefVisitor)
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct ScalarRef {
    asset: Option<Bitmap>,
    src: Option<PathBuf>,
    val: Option<u8>,
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
                formatter.write_str("hex string, path string, bitmap asset, or floating point value")
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

                Ok(Some(ScalarRef {
                    asset: None,
                    src: None,
                    val: Some((val * u8::MAX as f32) as _),
                }))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(ScalarRef {
                    asset: Some(asset),
                    src: None,
                    val: None,
                }))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if str.starts_with('#') {
                    if let Some(val) = ScalarRef::parse_hex(str) {
                        return Ok(Some(ScalarRef {
                            asset: None,
                            src: None,
                            val: Some(val),
                        }));
                    }
                }

                Ok(Some(ScalarRef {
                    asset: None,
                    src: Some(PathBuf::from(str)),
                    val: None,
                }))
            }
        }

        deserializer.deserialize_any(ScalarRefVisitor)
    }

    // TODO: Scalar parsing and error handling should be better and somewhere else
    fn parse_hex(val: &str) -> Option<u8> {
        match val.len() {
            2 => Some(u8::from_str_radix(&val[1..2].repeat(2), 16).unwrap()),
            3 => Some(u8::from_str_radix(&val[1..3], 16).unwrap()),
            _ => None,
        }
    }
}
