use {
    super::{
        asset::{Asset, Material as MaterialAsset},
        bake_bitmap,
        bitmap::pixels,
        get_filename_key, get_path,
    },
    crate::{color::{AlphaColor, MAGENTA}, pak::{id::MaterialId, BitmapBuf, BitmapFormat, MaterialDesc, PakBuf}},
    std::path::Path,
};

const DEFAULT_METALNESS: f32 = 0.5;
const DEFAULT_ROUGHNESS: f32 = 0.5;

/// Reads and processes 3D model material source files into an existing `.pak` file buffer.
pub fn bake_material<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    filename: P2,
    material: &MaterialAsset,
    mut pak: &mut PakBuf,
) -> MaterialId {
    // let key = get_filename_key(&project_dir, &filename);
    // if let Some(id) = pak.id(&key) {
    //     return id.as_material().unwrap();
    // }

    // info!("Processing asset: {}", key);

    // let dir = filename.as_ref().parent().unwrap();

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

    // // Pak this asset
    // pak.push_material(
    //     key,
    //     MaterialDesc {
    //         color,
    //         metal_rough,
    //         normal,
    //     },
    // )
    todo!();
}

fn create_bitmap(val: &[f32], height: usize, width: usize) -> Vec<u8> {
    val.repeat(width * height).iter().map(|val| (*val * 255f32) as u8).collect()
}
