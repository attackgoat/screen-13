use {
    super::{
        get_filename_key,
        pak_log::{LogId, PakLog},
        schema::{Asset, Language},
    },
    crate::pak::PakBuf,
    std::path::Path,
};

pub fn bake_lang<P1, P2>(
    pak: &mut PakBuf,
    project_dir: P1,
    src: P2,
    lang: &Language,
) where P1: AsRef<Path>, P2: AsRef<Path> {
    let asset = Asset::Language(loc_asset.clone());
    if log.contains(&asset) {
        return;
    } else {
        log.add(&asset, LogId::Locale(loc_asset.locale().to_owned()))
    }

    let key = get_filename_key(&project_dir, &asset_filename);
    info!("Baking language: {}", key);

    // Pak this asset
    pak.push_localization(loc_asset.locale().to_owned(), loc_asset.text().clone());
}
