use {
    super::{
        asset::{Asset, Material as MaterialAsset},
        bake_bitmap, get_filename_key, get_path,
    },
    crate::pak::{Material, MaterialId, PakBuf},
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

    let albedo_filename = get_path(dir, material.albedo(), &project_dir);
    let metal_filename = get_path(dir, material.metal(), &project_dir);
    let normal_filename = get_path(dir, material.normal(), &project_dir);

    let albedo = Asset::read(&albedo_filename).into_bitmap().unwrap();
    let metal = Asset::read(&metal_filename).into_bitmap().unwrap();
    let normal = Asset::read(&normal_filename).into_bitmap().unwrap();

    let albedo = bake_bitmap(&project_dir, albedo_filename, &albedo, &mut pak);
    let metal = bake_bitmap(&project_dir, metal_filename, &metal, &mut pak);
    let normal = bake_bitmap(&project_dir, normal_filename, &normal, &mut pak);

    // Pak this asset
    pak.push_material(
        key,
        Material {
            albedo,
            metal,
            normal,
        },
    )
}
