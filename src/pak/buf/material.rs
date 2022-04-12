use {
    super::{
        bitmap::Bitmap, file_key, is_toml,  parse_hex_color, parse_hex_scalar, Asset,
        Canonicalize, MaterialId, MaterialInfo,
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
        fmt::Formatter,
        num::FpCategory,
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use {super::Writer, parking_lot::Mutex, std::sync::Arc, tokio::runtime::Runtime};

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
    /// .. = { src = "file.png", format = "rgb" }
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

/// A reference to a `Bitmap` asset, `Bitmap` asset file, three channel image source file,
/// or single three channel color.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EmissiveRef {
    /// A `Bitmap` asset specified inline.
    Asset(Bitmap),

    /// A `Bitmap` asset file or image source file.
    Path(PathBuf),

    /// A single three channel color.
    Value([u8; 3]),
}

impl EmissiveRef {
    pub const WHITE: Self = Self::Value([u8::MAX; 3]);

    /// Deserialize from any of:
    ///
    /// val of [0.666, 0.733, 0.8]:
    /// .. = "#abc"
    /// .. = "#aabbcc"
    /// .. = [0.666, 0.733, 0.8]
    ///
    /// src of file.png:
    /// .. = "file.png"
    ///
    /// src of file.toml which must be a `Bitmap` asset:
    /// .. = "file.toml"
    ///
    /// src of a `Bitmap` asset:
    /// .. = { src = "file.png", format = "rgb" }
    fn de<'de, D>(deserializer: D) -> Result<Option<Self>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EmissiveRefVisitor;

        impl<'de> Visitor<'de> for EmissiveRefVisitor {
            type Value = Option<EmissiveRef>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("hex string, path string, bitmap asset, or seqeunce")
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let asset = Deserialize::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Some(EmissiveRef::Asset(asset)))
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
                    _ => panic!("Unexpected color length"),
                }

                Ok(Some(EmissiveRef::Value([
                    (val[0] * u8::MAX as f32) as u8,
                    (val[1] * u8::MAX as f32) as u8,
                    (val[2] * u8::MAX as f32) as u8,
                ])))
            }

            fn visit_str<E>(self, str: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if str.starts_with('#') {
                    if let Some(val) = parse_hex_color(str) {
                        assert_eq!(val[3], u8::MAX);

                        return Ok(Some(EmissiveRef::Value([val[0], val[1], val[2]])));
                    }
                }

                Ok(Some(EmissiveRef::Path(PathBuf::from(str))))
            }
        }

        deserializer.deserialize_any(EmissiveRefVisitor)
    }
}

impl Canonicalize for EmissiveRef {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        match self {
            Self::Asset(bitmap) => bitmap.canonicalize(project_dir, src_dir),
            Self::Path(src) => *src = Self::canonicalize_project_path(project_dir, src_dir, &src),
            _ => (),
        }
    }
}

impl Default for EmissiveRef {
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

    /// A `Bitmap` asset, `Bitmap` asset file, three channel image source file, or a single
    /// three channel color.
    #[serde(default, deserialize_with = "EmissiveRef::de")]
    pub emissive: Option<EmissiveRef>,

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
        rt: &Runtime,
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
        src: Option<impl AsRef<Path>>,
    ) -> anyhow::Result<MaterialId> {
        // Early-out if we have already baked this material
        let asset = self.clone().into();
        if let Some(id) = writer.lock().ctx.get(&asset) {
            return Ok(id.as_material().unwrap());
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

        let material_info = self.as_material_info(rt, writer, project_dir, src_dir)?;

        let mut writer = writer.lock();
        if let Some(id) = writer.ctx.get(&asset) {
            return Ok(id.as_material().unwrap());
        }

        Ok(writer.push_material(material_info, key))
    }

    #[cfg(feature = "bake")]
    fn as_material_info(
        &mut self,
        rt: &Runtime,
        writer: &Arc<Mutex<Writer>>,
        project_dir: impl AsRef<Path>,
        src_dir: impl AsRef<Path>,
    ) -> anyhow::Result<MaterialInfo> {
        let color = match &mut self.color {
            ColorRef::Asset(bitmap) => {
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();
                let mut bitmap = bitmap.clone();

                rt.spawn_blocking(move || {
                    bitmap
                        .bake(&writer, &project_dir)
                        .context("Unable to bake color asset bitmap")
                        .unwrap()
                })
            }
            ColorRef::Path(src) => {
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
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();

                rt.spawn_blocking(move || {
                    bitmap
                        .bake_from_source(&writer, &project_dir, Some(src))
                        .context("Unable to bake color asset bitmap from path")
                        .unwrap()
                })
            }
            ColorRef::Value(val) => {
                let writer = writer.clone();
                let val = *val;

                rt.spawn_blocking(move || {
                    let mut writer = writer.lock();
                    if let Some(id) = writer.ctx.get(&Asset::ColorRgba(val)) {
                        id.as_bitmap().unwrap()
                    } else {
                        let bitmap =
                            BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgba, 1, val);
                        writer.push_bitmap(bitmap, None)
                    }
                })
            }
        };

        let normal = match &mut self.normal {
            Some(NormalRef::Asset(bitmap)) => {
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();
                let mut bitmap = bitmap.clone().with_format(BitmapFormat::Rgb);

                rt.spawn_blocking(move || {
                    bitmap
                        .bake(&writer, &project_dir)
                        .context("Unable to bake normal asset bitmap")
                        .unwrap()
                })
            }
            Some(NormalRef::Path(src)) => {
                let src = src_dir
                    .as_ref()
                    .join(&src)
                    .canonicalize()
                    .context("Unable to canonicalize source path")?;
                let bitmap = if is_toml(&src) {
                    let mut bitmap = Asset::read(&src)
                        .context("Unable to read normal bitmap asset")?
                        .into_bitmap()
                        .expect("Source file should be a bitmap asset");
                    bitmap.canonicalize(&project_dir, &src_dir);
                    bitmap
                } else {
                    Bitmap::new(&src)
                };
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();

                rt.spawn_blocking(move || {
                    bitmap
                        .with_format(BitmapFormat::Rgb)
                        .bake_from_source(&writer, &project_dir, Some(src))
                        .context("Unable to bake normal asset bitmap from path")
                        .unwrap()
                })
            }
            None => {
                let writer = writer.clone();

                rt.spawn_blocking(move || {
                    let normal_val = [128, 128, 255];
                    let mut writer = writer.lock();
                    if let Some(id) = writer.ctx.get(&Asset::ColorRgb(normal_val)) {
                        id.as_bitmap().unwrap()
                    } else {
                        let bitmap =
                            BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgb, 1, normal_val);
                        writer.push_bitmap(bitmap, None)
                    }
                })
            }
        };

        let emissive = match &mut self.emissive {
            Some(EmissiveRef::Asset(bitmap)) => {
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();
                let mut bitmap = bitmap.clone().with_format(BitmapFormat::Rgb);

                rt.spawn_blocking(move || {
                    Some(
                        bitmap
                            .bake(&writer, &project_dir)
                            .context("Unable to bake emissive asset bitmap")
                            .unwrap(),
                    )
                })
            }
            Some(EmissiveRef::Path(src)) => {
                let src = src_dir
                    .as_ref()
                    .join(&src)
                    .canonicalize()
                    .context("Unable to canonicalize source path")?;
                let bitmap = if is_toml(&src) {
                    let mut bitmap = Asset::read(&src)
                        .context("Unable to read emissive bitmap asset")?
                        .into_bitmap()
                        .expect("Source file should be a bitmap asset");
                    bitmap.canonicalize(&project_dir, &src_dir);
                    bitmap
                } else {
                    Bitmap::new(&src)
                };
                let writer = writer.clone();
                let project_dir = project_dir.as_ref().to_path_buf();

                rt.spawn_blocking(move || {
                    Some(
                        bitmap
                            .with_format(BitmapFormat::Rgb)
                            .bake_from_source(&writer, &project_dir, Some(src))
                            .context("Unable to bake emissive asset bitmap from path")
                            .unwrap(),
                    )
                })
            }
            Some(EmissiveRef::Value(val)) => {
                let writer = writer.clone();
                let val = *val;

                rt.spawn_blocking(move || {
                    let mut writer = writer.lock();
                    Some(if let Some(id) = writer.ctx.get(&Asset::ColorRgb(val)) {
                        id.as_bitmap().unwrap()
                    } else {
                        let bitmap = BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgb, 1, val);
                        writer.push_bitmap(bitmap, None)
                    })
                })
            }
            None => rt.spawn_blocking(|| None),
        };

        let displacement = self.displacement.clone();
        let metal = self.metal.clone();
        let rough = self.rough.clone();
        let params_asset = Asset::MaterialParams(MaterialParams {
            displacement,
            metal,
            rough,
        });
        let params = {
            let project_dir = project_dir.as_ref().to_path_buf();
            let src_dir = src_dir.as_ref().to_path_buf();
            let writer = writer.clone();
            let displacement = self.displacement.clone();
            let metal = self.metal.clone();
            let rough = self.rough.clone();

            rt.spawn_blocking(move || {
                if let Some(id) = writer.lock().ctx.get(&params_asset) {
                    return id.as_bitmap().unwrap();
                }

                let mut metal_image = DynamicImage::ImageLuma8(
                    Self::scalar_ref_into_gray_image(&metal, &project_dir, &src_dir)
                        .context("Unable to create metal bitmap buf")
                        .unwrap(),
                );
                let mut rough_image = DynamicImage::ImageLuma8(
                    Self::scalar_ref_into_gray_image(&rough, &project_dir, &src_dir)
                        .context("Unable to create rough bitmap buf")
                        .unwrap(),
                );
                let mut displacement_image = DynamicImage::ImageLuma8(
                    Self::scalar_ref_into_gray_image(&displacement, &project_dir, &src_dir)
                        .context("Unable to create displacement bitmap buf")
                        .unwrap(),
                );

                let width = metal_image
                    .width()
                    .max(rough_image.width())
                    .max(displacement_image.width());
                let height = metal_image
                    .height()
                    .max(rough_image.height())
                    .max(displacement_image.height());

                if metal_image.width() != width || metal_image.height() != height {
                    let filter_ty = if metal_image.width() == 1 && metal_image.height() == 1 {
                        FilterType::Nearest
                    } else {
                        FilterType::CatmullRom
                    };

                    metal_image = metal_image.resize_to_fill(width, height, filter_ty);
                }

                if rough_image.width() != width || rough_image.height() != height {
                    let filter_ty = if rough_image.width() == 1 && rough_image.height() == 1 {
                        FilterType::Nearest
                    } else {
                        FilterType::CatmullRom
                    };

                    rough_image = rough_image.resize_to_fill(width, height, filter_ty);
                }

                if displacement_image.width() != width || displacement_image.height() != height {
                    let filter_ty =
                        if displacement_image.width() == 1 && displacement_image.height() == 1 {
                            FilterType::Nearest
                        } else {
                            FilterType::CatmullRom
                        };

                    displacement_image =
                        displacement_image.resize_to_fill(width, height, filter_ty);
                }

                let mut params = Vec::with_capacity(
                    (2 * width * height) as usize
                        + displacement
                            .as_ref()
                            .map(|_| width * height)
                            .unwrap_or_default() as usize,
                );

                for y in 0..height {
                    for x in 0..width {
                        params.push(metal_image.get_pixel(x, y).0[0]);
                        params.push(rough_image.get_pixel(x, y).0[0]);

                        if displacement.is_some() {
                            params.push(displacement_image.get_pixel(x, y).0[0]);
                        }
                    }
                }

                let mut writer = writer.lock();
                let res = if let Some(id) = writer.ctx.get(&params_asset) {
                    id.as_bitmap().unwrap()
                } else {
                    let params = BitmapBuf::new(
                        BitmapColor::Linear,
                        if displacement.is_none() {
                            BitmapFormat::Rg
                        } else {
                            BitmapFormat::Rgb
                        },
                        width,
                        params,
                    );
                    writer.push_bitmap(params, None)
                };

                res
            })
        };

        let (color, emissive, normal, params) = rt.block_on(async move {
            let color = color.await.unwrap();
            let emissive = emissive.await.unwrap();
            let normal = normal.await.unwrap();
            let params = params.await.unwrap();

            (color, emissive, normal, params)
        });

        Ok(MaterialInfo {
            color,
            emissive,
            normal,
            params,
        })
    }

    fn scalar_ref_into_gray_image(
        scalar: &Option<ScalarRef>,
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

        if let Some(emissive) = self.emissive.as_mut() {
            emissive.canonicalize(&project_dir, &src_dir);
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
            emissive: None,
            metal: None,
            normal: None,
            rough: None,
        }
    }
}

/// Holds a description of data used while baking materials. This is for caching.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct MaterialParams {
    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub displacement: Option<ScalarRef>,

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub metal: Option<ScalarRef>,

    /// A `Bitmap` asset, `Bitmap` asset file, single channel image source file, or a single
    /// normalized value.
    #[serde(default, deserialize_with = "ScalarRef::de")]
    pub rough: Option<ScalarRef>,
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
    /// .. = { src = "file.png", format = "rgb" }
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
    /// .. = { src = "file.png", format = "r" }
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
