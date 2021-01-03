use {
    super::{
        asset::{Asset, Material as MaterialAsset},
        bake_bitmap,
        bitmap::pixels,
        get_filename_key, get_path,
    },
    crate::pak::{id::MaterialId, Bitmap, BitmapFormat, MaterialDesc, PakBuf},
    std::path::Path,
};

pub fn bake_material<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    filename: P2,
    material: &MaterialAsset,
    mut pak: &mut PakBuf,
) -> MaterialId {
    let key = get_filename_key(&project_dir, &filename);
    if let Some(id) = pak.id(&key) {
        return id.as_material().unwrap();
    }

    info!("Processing asset: {}", key);

    let dir = filename.as_ref().parent().unwrap();

    let color_filename = get_path(dir, material.color(), &project_dir);
    let metal_filename = get_path(dir, material.metal_src(), &project_dir);
    let normal_filename = get_path(dir, material.normal(), &project_dir);
    let rough_filename = get_path(dir, material.rough_src(), &project_dir);

    let color = Asset::read(&color_filename).into_bitmap().unwrap();
    let normal = Asset::read(&normal_filename).into_bitmap().unwrap();

    let color = bake_bitmap(&project_dir, color_filename, &color, &mut pak);
    let normal = bake_bitmap(&project_dir, normal_filename, &normal, &mut pak);

    // TODO: "Entertaining" key format which is temporary because it starts with a period
    let metal_rough_key = format!(
        ".materal-metal-rough:{}+{}",
        metal_filename.display(),
        rough_filename.display()
    );
    let metal_rough = if let Some(id) = pak.id(&metal_rough_key) {
        id.as_bitmap().unwrap()
    } else {
        let (metal_width, metal_pixels) = pixels(metal_filename, BitmapFormat::R);
        let (rough_width, rough_pixels) = pixels(rough_filename, BitmapFormat::R);

        // The metalness/roughness map source art must be of equal size
        assert_eq!(metal_width, rough_width);
        assert_eq!(metal_pixels.len(), rough_pixels.len());

        let mut metal_rough_pixels = Vec::with_capacity(metal_pixels.len() * 2);

        unsafe {
            metal_rough_pixels.set_len(metal_pixels.len() * 2);
        }

        for idx in 0..metal_pixels.len() {
            metal_rough_pixels[idx * 2] = metal_pixels[idx];
            metal_rough_pixels[idx * 2 + 1] = rough_pixels[idx];
        }

        // Pak this asset
        let metal_rough = Bitmap::new(BitmapFormat::Rg, metal_width as u16, metal_rough_pixels);
        pak.push_bitmap(metal_rough_key, metal_rough)
    };

    // Pak this asset
    pak.push_material(
        key,
        MaterialDesc {
            color,
            metal_rough,
            normal,
        },
    )
}
