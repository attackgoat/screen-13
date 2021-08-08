use crate::bake::asset::Canonicalize;

use {
    super::{
        asset::{Asset, Bitmap, ColorRef, Material, NormalRef, ScalarRef},
        bake_bitmap,
        bitmap::pixels,
        get_filename_key, parent,
    },
    crate::{
        bake::is_toml,
        color::{AlphaColor, MAGENTA},
        pak::{
            id::{Id, MaterialId},
            BitmapBuf, BitmapFormat, MaterialDesc, PakBuf,
        },
    },
    std::{collections::HashMap, path::Path},
};

const DEFAULT_METALNESS: f32 = 0.5;
const DEFAULT_ROUGHNESS: f32 = 0.5;

/// Reads and processes 3D model material source files into an existing `.pak` file buffer.
pub fn bake_material<P1, P2>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P1,
    src: Option<P2>,
    material: &Material,
) -> MaterialId
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    // Early-out if we have this asset in our context
    let context_key = material.clone().into();
    if let Some(id) = context.get(&context_key) {
        return id.as_material().unwrap();
    }

    // If a source is given it will be available as a key inside the .pak (sources are not
    // given if the asset is specified inline - those are only available in the .pak via ID)
    let key = src.as_ref().map(|src| get_filename_key(&project_dir, &src));
    if let Some(key) = &key {
        // This material will be accessible using this key
        info!("Baking material: {}", key);
    } else {
        // This model will only be accessible using the ID
        info!("Baking material: (inline)");
    }

    // Pak this asset and add it to the context
    let buf = bake(context, pak, project_dir, material);
    let id = pak.push_material(key, buf);
    context.insert(context_key, id.into());
    id
}

fn bake<P>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P,
    material: &Material,
) -> MaterialDesc
where
    P: AsRef<Path>,
{
    let color: Asset = match material.color() {
        ColorRef::Asset(bitmap) => bitmap.clone().into(),
        ColorRef::Path(src) => if is_toml(&src) {
            let mut bitmap = Asset::read(src).into_bitmap().unwrap();
            let src_dir = parent(&project_dir, src);
            bitmap.canonicalize(project_dir, src_dir);
            bitmap
        } else {
            Bitmap::new(src)
        }
        .into(),
        ColorRef::Value(val) => (*val).into(),
    };

    // // Gets the bitmap ID of either the source file or a new bitmap of just one color
    // let color = if let Some(src) = material.color_src() {
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
    //     let val = material.color_val().unwrap_or_else(|| {
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
    // let normal = if let Some(src) = material.normal() {
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

    // let metal_key = if let Some(src) = material.metal_src() {
    //     format!("file:{}", src.display())
    // } else {
    //     format!("scalar:{}", material.metal_val().unwrap_or(DEFAULT_METALNESS))
    // };
    // let rough_key = if let Some(src) = material.metal_src() {
    //     format!("file:{}", src.display())
    // } else {
    //     format!("scalar:{}", material.metal_val().unwrap_or(DEFAULT_METALNESS))
    // };
    // let metal_rough_key = format!(
    //     ".materal-metal-rough:{} {}",
    //     &metal_key,
    //     &rough_key
    // );
    // if let Some(id) = pak.id(&normal_key) {
    //     id.as_material().unwrap()
    // } else {
    //     let metal = if let Some(src) = material.metal_src() {
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

    // let metal_filename = get_path(dir, material.metal_src(), &project_dir);
    // let rough_filename = get_path(dir, material.rough_src(), &project_dir);

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

fn create_bitmap(val: &[f32], height: usize, width: usize) -> Vec<u8> {
    val.repeat(width * height)
        .iter()
        .map(|val| (*val * 255f32) as u8)
        .collect()
}
