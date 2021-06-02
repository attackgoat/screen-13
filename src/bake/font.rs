use {
    super::{asset::Font as FontAsset, get_filename_key, get_path},
    crate::pak::{id::FontId, PakBuf},
    fontdue::{Font, FontSettings},
    std::{fs::read, path::Path},
};

/// Reads and processes scalable font source files into an existing `.pak` file buffer.
pub fn bake_font<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    font_asset: &FontAsset,
    pak: &mut PakBuf,
) -> FontId {
    let key = get_filename_key(&project_dir, &asset_filename);
    if let Some(id) = pak.id(&key) {
        return id.as_font().unwrap();
    }

    info!("Processing asset: {}", key);

    assert!(font_asset.scale() > 0.0);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let src_filename = get_path(&dir, font_asset.src(), project_dir);
    let src_file = read(&src_filename).unwrap();

    let font = Font::from_bytes(
        src_file,
        FontSettings {
            enable_offset_bounding_box: true,
            collection_index: font_asset.collection_index(),
            scale: font_asset.scale(),
        },
    )
    .unwrap();

    // Pak this asset
    pak.push_font(key, font)
}
