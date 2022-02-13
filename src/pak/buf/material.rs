use {
    super::{
        bitmap::Bitmap, file_key, parse_hex_color, parse_hex_scalar, Asset, Canonicalize, Id,
        MaterialId, MaterialInfo,
    },
    log::info,
    serde::{
        de::{
            value::{MapAccessDeserializer, SeqAccessDeserializer},
            MapAccess, SeqAccess, Visitor,
        },
        Deserialize, Deserializer,
    },
    std::{
        collections::HashMap,
        fmt::Formatter,
        io::Error,
        num::FpCategory,
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use super::Writer;

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
    pub const WHITE: Self = Self::Value([u8::MAX; 4]);

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

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
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
                E: serde::de::Error,
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
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
            _ => (),
        }
    }
}

impl Default for ColorRef {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Holds a description of data used for model rendering.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Material {
    #[serde(default, deserialize_with = "ColorRef::de")]
    color: ColorRef,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    displacement: Option<ScalarRef>,

    double_sided: Option<bool>,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    metal: Option<ScalarRef>,

    #[serde(default, deserialize_with = "NormalRef::de")]
    normal: Option<NormalRef>,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    rough: Option<ScalarRef>,
}

impl Material {
    // const DEFAULT_METALNESS: f32 = 0.5;
    // const DEFAULT_ROUGHNESS: f32 = 0.5;

    #[allow(unused)]
    pub(crate) fn new<P>(src: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            color: ColorRef::Path(src.as_ref().to_owned()),
            ..Default::default()
        }
    }

    /// Reads and processes 3D model material source files into an existing `.pak` file buffer.
    #[allow(unused)]
    #[cfg(feature = "bake")]
    pub(super) fn bake(
        &self,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> Result<MaterialId, Error> {
        // Early-out if we have already baked this material
        if let Some(h) = writer.ctx.get(&self.clone().into()) {
            return Ok(h.as_material().unwrap());
        }

        // If a source is given it will be available as a key inside the .pak (sources are not
        // given if the asset is specified inline - those are only available in the .pak via ID)
        let key = src.as_ref().map(|src| file_key(&project_dir, &src));
        if let Some(key) = &key {
            // This material will be accessible using this key
            info!("Baking material: {}", key);
        } else {
            // This model will only be accessible using the ID
            info!("Baking material: (inline)");
        }

        let material_info = self.to_material_info(writer, project_dir)?;

        Ok(writer.push_material(material_info, key))
    }

    #[cfg(feature = "bake")]
    fn to_material_info(
        &self,
        _writer: &mut Writer,
        _project_dir: impl AsRef<Path>,
    ) -> Result<MaterialInfo, Error> {
        //     let color: Asset = match self.color() {
        //         ColorRef::Asset(bitmap) => bitmap.clone().into(),
        //         ColorRef::Path(src) => if is_toml(&src) {
        //             let mut bitmap = Asset::read(src)?.into_bitmap().unwrap();
        //             let src_dir = parent(&project_dir, src);
        //             bitmap.canonicalize(project_dir, src_dir);
        //             bitmap
        //         } else {
        //             Bitmap::new(src)
        //         }
        //         .into(),
        //         ColorRef::Value(val) => (*val).into(),
        //     };

        // // Gets the bitmap ID of either the source file or a new bitmap of just one color
        // let color = if let Some(src) = self.color_src() {
        //     let color_filename = get_path(dir, src, &project_dir);
        //     if let Some("toml") = color_filename.extension()
        //     .map(|ext| ext.to_str())
        //     .flatten()
        //     .map(|ext| ext.to_lowercase())
        //     .as_deref() {
        //         let color = Asset::read(&color_filename).into_bitmap().unwrap();
        //         bake_bitmap(&project_dir, color_filename, &color, &mut pak)
        //     } else {

        //     }
        // } else {
        //     let val = self.color_val().unwrap_or_else(|| {
        //         let color: AlphaColor = MAGENTA.into();
        //         color.into()
        //     });
        //     let color_key = format!(
        //         ".materal-color-val:{:?}",
        //         val,
        //     );
        //     if let Some(id) = pak.id(&color_key) {
        //         id.as_bitmap().unwrap()
        //     } else {
        //         let pixels = create_bitmap(&val, 16, 16);
        //         let bitmap = BitmapBuf::new(BitmapFormat::Rgba, 16, pixels);
        //         pak.push_bitmap(key, bitmap)
        //     }
        // };

        // // Gets the bitmap ID of either the source file or a new bitmap of just one color
        // let normal = if let Some(src) = self.normal() {
        //     let normal_filename = get_path(dir, src, &project_dir);
        //     let normal = Asset::read(&normal_filename).into_bitmap().unwrap();
        //     bake_bitmap(&project_dir, normal_filename, &normal, &mut pak)
        // } else {
        //     // TODO: Correct normal map color!
        //     let val: [f32; 3] = [0.0, 0.0, 0.0];
        //     let normal_key = format!(
        //         ".materal-normal-val:{:?}",
        //         val,
        //     );
        //     if let Some(id) = pak.id(&normal_key) {
        //         id.as_bitmap().unwrap()
        //     } else {
        //         let pixels = create_bitmap(&val, 16, 16);
        //         let bitmap = BitmapBuf::new(BitmapFormat::Rgb, 16, pixels);
        //         pak.push_bitmap(key, bitmap)
        //     }
        // };

        // let metal_key = if let Some(src) = self.metal_src() {
        //     format!("file:{}", src.display())
        // } else {
        //     format!("scalar:{}", self.metal_val().unwrap_or(DEFAULT_METALNESS))
        // };
        // let rough_key = if let Some(src) = self.metal_src() {
        //     format!("file:{}", src.display())
        // } else {
        //     format!("scalar:{}", self.metal_val().unwrap_or(DEFAULT_METALNESS))
        // };
        // let metal_rough_key = format!(
        //     ".materal-metal-rough:{} {}",
        //     &metal_key,
        //     &rough_key
        // );
        // if let Some(id) = pak.id(&normal_key) {
        //     id.self().unwrap()
        // } else {
        //     let metal = if let Some(src) = self.metal_src() {
        //         let metal_filename = get_path(dir, src, &project_dir);
        //         let metal = Asset::read(&normal_filename).into_bitmap().unwrap();
        //         bake_bitmap(&project_dir, normal_filename, &normal, &mut pak)
        //     } else {
        //         // TODO: Correct normal map color!
        //         let val: [f32; 3] = [0.0, 0.0, 0.0];
        //         let normal_key = format!(
        //             ".materal-normal:{:?}",
        //             val,
        //         );
        //         if let Some(id) = pak.id(&normal_key) {
        //             id.as_bitmap().unwrap()
        //         } else {
        //             let pixels = create_bitmap(&val, 16, 16);
        //             let bitmap = BitmapBuf::new(BitmapFormat::Rgb, 16, pixels);
        //             pak.push_bitmap(key, bitmap)
        //         }
        //     };

        // let metal_filename = get_path(dir, self.metal_src(), &project_dir);
        // let rough_filename = get_path(dir, self.rough_src(), &project_dir);

        // // TODO: "Entertaining" key format which is temporary because it starts with a period

        // let metal_rough = if let Some(id) = pak.id(&metal_rough_key) {
        //     id.as_bitmap().unwrap()
        // } else {
        //     let (metal_width, metal_pixels) = pixels(metal_filename, BitmapFormat::R);
        //     let (rough_width, rough_pixels) = pixels(rough_filename, BitmapFormat::R);

        //     // The metalness/roughness map source art must be of equal size
        //     assert_eq!(metal_width, rough_width);
        //     assert_eq!(metal_pixels.len(), rough_pixels.len());

        //     let mut metal_rough_pixels = Vec::with_capacity(metal_pixels.len() * 2);

        //     unsafe {
        //         metal_rough_pixels.set_len(metal_pixels.len() * 2);
        //     }

        //     for idx in 0..metal_pixels.len() {
        //         metal_rough_pixels[idx * 2] = metal_pixels[idx];
        //         metal_rough_pixels[idx * 2 + 1] = rough_pixels[idx];
        //     }

        //     // Pak this asset
        //     let metal_rough = BitmapBuf::new(BitmapFormat::Rg, metal_width as u16, metal_rough_pixels);
        //     pak.push_bitmap(metal_rough_key, metal_rough)
        // };

        //     MaterialDesc {
        //         color,
        //         metal_rough,
        //         normal,
        //     },

        todo!()
    }

    /// A `Bitmap` asset, `Bitmap` asset file, three or four channel image source file, or single
    /// four channel color.
    #[allow(unused)]
    pub fn color(&self) -> &ColorRef {
        &self.color
    }

    #[allow(unused)]
    fn create_bitmap(val: &[f32], height: usize, width: usize) -> Vec<u8> {
        val.repeat(width * height)
            .iter()
            .map(|val| (*val * u8::MAX as f32) as u8)
            .collect()
    }

    /// Whether or not the model will be rendered with back faces also enabled.
    #[allow(unused)]
    pub fn double_sided(&self) -> bool {
        self.double_sided.unwrap_or_default()
    }

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[allow(unused)]
    pub fn metal(&self) -> Option<&ScalarRef> {
        self.metal.as_ref()
    }

    /// A bitmap asset, bitmap asset file, or a three channel image.
    #[allow(unused)]
    pub fn normal(&self) -> Option<&NormalRef> {
        self.normal.as_ref()
    }

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[allow(unused)]
    pub fn rough(&self) -> Option<&ScalarRef> {
        self.rough.as_ref()
    }
}

impl Canonicalize for Material {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.color.canonicalize(&project_dir, &src_dir);

        if let Some(metal) = self.metal.as_mut() {
            metal.canonicalize(&project_dir, &src_dir);
        }

        if let Some(normal) = self.normal.as_mut() {
            normal.canonicalize(&project_dir, &src_dir);
        }

        if let Some(rough) = self.rough.as_mut() {
            rough.canonicalize(&project_dir, &src_dir);
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: ColorRef::WHITE,
            displacement: None,
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

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
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
                E: serde::de::Error,
            {
                Ok(Some(NormalRef::Path(PathBuf::from(str))))
            }
        }

        deserializer.deserialize_any(NormalRefVisitor)
    }
}

impl Canonicalize for NormalRef {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
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

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter
                    .write_str("hex string, path string, bitmap asset, or floating point value")
            }

            fn visit_f64<E>(self, val: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
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
                E: serde::de::Error,
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
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
            _ => (),
        }
    }
}
