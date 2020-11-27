use {
    super::{
        asset::{Asset, Material as MaterialAsset},
        bake_bitmap, get_filename_key, get_path,
        pak_log::{Id, PakLog},
    },
    crate::pak::{Material, MaterialId, PakBuf},
    std::path::Path,
};

pub fn bake_material<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    filename: P2,
    material: &MaterialAsset,
    mut pak: &mut PakBuf,
    mut log: &mut PakLog,
) -> MaterialId {
    let dir = filename.as_ref().parent().unwrap();

    let albedo_filename = get_path(dir, material.albedo());
    let metal_filename = get_path(dir, material.metal());
    let normal_filename = get_path(dir, material.normal());
    let proto = Asset::Material(MaterialAsset::new(
        &albedo_filename,
        &normal_filename,
        &metal_filename,
    ));
    if log.contains(&proto) {
        match log.get(&proto).unwrap() {
            Id::Material(id) => return id,
            _ => panic!(),
        }
    }

    let key = get_filename_key(&project_dir, &filename);

    info!("Processing asset: {}", key);

    let albedo = Asset::read(&albedo_filename).into_bitmap().unwrap();
    let metal = Asset::read(&metal_filename).into_bitmap().unwrap();
    let normal = Asset::read(&normal_filename).into_bitmap().unwrap();

    let albedo = bake_bitmap(&project_dir, albedo_filename, &albedo, &mut pak, &mut log);
    let metal = bake_bitmap(&project_dir, metal_filename, &metal, &mut pak, &mut log);
    let normal = bake_bitmap(&project_dir, normal_filename, &normal, &mut pak, &mut log);

    // Pak and log this asset
    let id = pak.push_material(
        key,
        Material {
            albedo,
            metal,
            normal,
        },
    );
    log.add(&proto, id);

    id
}
