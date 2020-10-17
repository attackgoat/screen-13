use {
    super::{asset::Scene, get_filename_key},
    crate::pak::{PakBuf, SceneRef},
    std::path::Path,
};

pub fn bake_scene<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    value: &Scene,
    pak: &mut PakBuf,
) -> Vec<String> {
    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    let mut keys = vec![];
    let mut scene = vec![];
    for r in value.refs() {
        println!("Ref: {} ({})", r.id().unwrap_or(""), r.key().unwrap_or(""));

        // all tags must be lower case (no localized text!)
        let mut tags = vec![];
        for tag in r.tags() {
            tags.push(tag.as_str().to_lowercase());
        }

        if r.key().is_some() && !r.key().unwrap().is_empty() {
            keys.push(r.key().unwrap().to_owned());
        }

        scene.push(SceneRef::new(
            r.id().map(|id| id.to_owned()),
            r.key().map(|key| key.to_owned()),
            r.position(),
            r.rotation(),
            tags,
        ));
    }

    // Pak this asset
    pak.push_scene(key, scene);
    keys
}
