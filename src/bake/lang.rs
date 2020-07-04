use {
    super::{
        pak_log::{LogId, PakLog},
        schema::{Asset, LanguageAsset},
    },
    crate::pak::PakBuf,
    std::path::Path,
};

#[cfg(debug_assertions)]
use super::get_filename_key;

pub fn bake_lang<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    loc_asset: &LanguageAsset,
    pak: &mut PakBuf,
    log: &mut PakLog,
) {
    let asset = Asset::Language(loc_asset.clone());
    if log.contains(&asset) {
        return;
    } else {
        log.add(&asset, LogId::Locale(loc_asset.locale().to_owned()))
    }

    let key = get_filename_key(&project_dir, &asset_filename);
    info!("Processing asset: {}", key);

    // Pak this asset
    pak.push_localization(loc_asset.locale().to_owned(), loc_asset.text().clone());
}
