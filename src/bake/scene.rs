use {
    super::{get_filename_key, SceneAsset},
    crate::pak::{PakBuf, SceneRef},
    std::path::Path,
};

pub fn bake_scene<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    value: &SceneAsset,
    pak: &mut PakBuf,
) -> Vec<String> {
    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    let mut keys = vec![];
    let mut scene = vec![];
    for item in value.items() {
        let mut tags = vec![];
        for tag in item.tags() {
            tags.push(tag.as_str().to_lowercase());
        }

        if !item.key.is_empty() {
            keys.push(item.key.clone());
        }

        scene.push(SceneRef::new(
            item.id.clone(),
            item.key.clone(),
            item.position(),
            item.roll_pitch_yaw(),
            tags,
        ));
    }

    // Pak this asset
    pak.push_scene(key, scene);
    keys
}
