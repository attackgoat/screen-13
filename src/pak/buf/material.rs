use {
    super::{
        bitmap::Bitmap, file_key, is_toml, parent, parse_hex_color, parse_hex_scalar, Asset,
        Canonicalize, Id, MaterialId, MaterialInfo,
    },
    crate::pak::{BitmapBuf, BitmapColor, BitmapFormat},
    anyhow::Context as _,
    image::{imageops::FilterType, DynamicImage, GenericImageView, GrayImage},
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
        num::FpCategory,
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use super::Writer;

/// A reference to a `Bitmap` asset, `Bitmap` asset file, three or four channel image source file,
/// or single four channel color.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Material {
    /// A `Bitmap` asset, `Bitmap` asset file, three or four channel image source file, or single
    /// four channel color.
    #[serde(default, deserialize_with = "ColorRef::de")]
    pub color: ColorRef,

    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub displacement: Option<ScalarRef>,

    /// Whether or not the model will be rendered with back faces also enabled.
    pub double_sided: Option<bool>,

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub metal: Option<ScalarRef>,

    /// A bitmap asset, bitmap asset file, or a three channel image.
    #[serde(default, deserialize_with = "NormalRef::de")]
    pub normal: Option<NormalRef>,

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub rough: Option<ScalarRef>,
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
    #[cfg(feature = "bake")]
    pub(super) fn bake(
        &mut self,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> anyhow::Result<MaterialId> {
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

        let material_info = self.to_material_info(writer, project_dir, src_dir)?;

        Ok(writer.push_material(material_info, key))
    }

    #[cfg(feature = "bake")]
    fn to_material_info(
        &mut self,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
    ) -> anyhow::Result<MaterialInfo> {
        let color = match &mut self.color {
            ColorRef::Asset(bitmap) => bitmap
                .bake(writer, &project_dir)
                .context("Unable to bake color asset bitmap")?,
            ColorRef::Path(src) => {
                info!("src_dir = {}", src_dir.as_ref().display());
                info!("src = {}", src.display());

                let src = src_dir
                    .as_ref()
                    .join(&src)
                    .canonicalize()
                    .context("Unable to canonicalize source path")?;
                let mut bitmap = if is_toml(&src) {
                    let mut bitmap = Asset::read(&src)
                        .context("Unable to read color bitmap asset")?
                        .into_bitmap()
                        .expect("Source file should be a bitmap asset");
                    bitmap.canonicalize(&project_dir, &src_dir);
                    bitmap
                } else {
                    Bitmap::new(&src)
                };
                bitmap
                    .bake_from_source(writer, &project_dir, Some(src))
                    .context("Unable to bake color asset bitmap from path")?
            }
            ColorRef::Value(val) => {
                let bitmap =
                    BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgba, 1, val.to_vec());
                writer.push_bitmap(bitmap, None)
            }
        };

        let normal = match &mut self.normal {
            Some(NormalRef::Asset(bitmap)) => bitmap
                .bake(writer, &project_dir)
                .context("Unable to bake normal asset bitmap")?,
            Some(NormalRef::Path(src)) => {
                let src = src_dir
                    .as_ref()
                    .join(&src)
                    .canonicalize()
                    .context("Unable to canonicalize source path")?;
                let mut bitmap = if is_toml(&src) {
                    let mut bitmap = Asset::read(&src)
                        .context("Unable to read normal bitmap asset")?
                        .into_bitmap()
                        .expect("Source file should be a bitmap asset");
                    bitmap.canonicalize(&project_dir, &src_dir);
                    bitmap
                } else {
                    Bitmap::new(&src)
                };
                bitmap
                    .bake_from_source(writer, &project_dir, Some(src))
                    .context("Unable to bake normal asset bitmap from path")?
            }
            None => {
                let bitmap =
                    BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgb, 1, [128, 128, 255]);
                writer.push_bitmap(bitmap, None)
            }
        };

        let mut metal = DynamicImage::ImageLuma8(
            Self::scalar_buf_into_gray_image(&mut self.metal, &project_dir, &src_dir)
                .context("Unable to create metal bitmap buf")?,
        );
        let mut rough = DynamicImage::ImageLuma8(
            Self::scalar_buf_into_gray_image(&mut self.rough, &project_dir, &src_dir)
                .context("Unable to create rough bitmap buf")?,
        );
        let mut displacement = DynamicImage::ImageLuma8(
            Self::scalar_buf_into_gray_image(&mut self.displacement, &project_dir, &src_dir)
                .context("Unable to create displacement bitmap buf")?,
        );

        let width = metal.width().max(rough.width().max(displacement.width()));
        let height = metal
            .height()
            .max(rough.height().max(displacement.height()));

        if metal.width() != width || metal.height() != height {
            metal = metal.resize_to_fill(width, height, FilterType::CatmullRom);
        }

        if rough.width() != width || rough.height() != height {
            rough = rough.resize_to_fill(width, height, FilterType::CatmullRom);
        }

        if displacement.width() != width || displacement.height() != height {
            displacement = displacement.resize_to_fill(width, height, FilterType::CatmullRom);
        }

        let mut params = Vec::with_capacity(
            (2 * width * height) as usize
                + self
                    .displacement
                    .as_ref()
                    .map(|_| width * height)
                    .unwrap_or_default() as usize,
        );

        for y in 0..height {
            for x in 0..width {
                params.push(metal.get_pixel(x, y).0[0]);
                params.push(rough.get_pixel(x, y).0[0]);

                if self.displacement.is_some() {
                    params.push(displacement.get_pixel(x, y).0[0]);
                }
            }
        }

        let params = BitmapBuf::new(
            BitmapColor::Linear,
            if self.displacement.is_none() {
                BitmapFormat::Rg
            } else {
                BitmapFormat::Rgb
            },
            width,
            params,
        );
        let params = writer.push_bitmap(params, None);

        Ok(MaterialInfo {
            color,
            normal,
            params,
        })
    }

    fn scalar_buf_into_gray_image(
        scalar: &mut Option<ScalarRef>,
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
    ) -> anyhow::Result<GrayImage> {
        let bitmap = match scalar {
            Some(ScalarRef::Asset(bitmap)) => bitmap
                .as_bitmap_buf()
                .context("Unable to create bitmap buf from scalar bitmap asset")?,
            Some(ScalarRef::Path(src)) => {
                let src = src_dir
                    .as_ref()
                    .join(&src)
                    .canonicalize()
                    .context("Unable to canonicalize source path")?;
                if is_toml(&src) {
                    let mut bitmap = Asset::read(&src)?
                        .into_bitmap()
                        .expect("Source file should be a bitmap asset");
                    bitmap.canonicalize(&project_dir, src_dir);
                    bitmap
                } else {
                    Bitmap::new(&src)
                }
            }
            .as_bitmap_buf()
            .context("Unable to create bitmap buf")?,
            Some(ScalarRef::Value(val)) => {
                BitmapBuf::new(BitmapColor::Linear, BitmapFormat::R, 1, [*val])
            }
            None => BitmapBuf::new(BitmapColor::Linear, BitmapFormat::R, 1, [128]),
        };
        let image =
            GrayImage::from_raw(bitmap.width, bitmap.height(), bitmap.pixels().to_vec()).unwrap();

        Ok(image)
    }
}

impl Canonicalize for Material {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.color.canonicalize(&project_dir, &src_dir);

        if let Some(displacement) = self.displacement.as_mut() {
            displacement.canonicalize(&project_dir, &src_dir);
        }

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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
