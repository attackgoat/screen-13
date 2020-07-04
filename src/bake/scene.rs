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

        scene.push(SceneRef {
            id: item.id.clone(),
            key: item.key.clone(),
            position: (
                item.position().x(),
                item.position().y(),
                item.position().z(),
            ),
            roll_pitch_yaw: (
                item.roll_pitch_yaw().x(),
                item.roll_pitch_yaw().y(),
                item.roll_pitch_yaw().z(),
            ),
            tags,
        })
    }

    // Pak this asset
    pak.push_scene(key, scene);
    keys
}
