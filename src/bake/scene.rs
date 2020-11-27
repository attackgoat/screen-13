use {
    super::{
        asset::Scene as SceneAsset, bake_material, bake_model, get_filename_key, get_path, Asset,
        PakLog,
    },
    crate::pak::{scene::Instance, PakBuf, Scene, SceneId},
    std::path::Path,
};

pub fn bake_scene<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    filename: P2,
    asset: &SceneAsset,
    mut pak: &mut PakBuf,
    mut log: &mut PakLog,
) -> SceneId {
    let dir = filename.as_ref().parent().unwrap();
    let key = get_filename_key(&project_dir, &filename);

    info!("Processing asset: {}", key);

    let mut refs = vec![];
    for scene_ref in asset.refs() {
        // all tags must be lower case (no localized text!)
        let mut tags = vec![];
        for tag in scene_ref.tags() {
            let baked = tag.as_str().trim().to_lowercase();
            if let Err(idx) = tags.binary_search(&baked) {
                tags.insert(idx, baked);
            }
        }

        let material = scene_ref.material().map(|src| {
            let src = get_path(&dir, src);
            let material = Asset::read(&src).into_material().unwrap();
            bake_material(&project_dir, src, &material, &mut pak, &mut log)
        });

        let model = scene_ref.model().map(|src| {
            let src = get_path(&dir, src);
            let model = Asset::read(&src).into_model().unwrap();
            bake_model(&project_dir, src, &model, &mut pak, &mut log)
        });

        refs.push(Instance {
            id: scene_ref.id().map(|id| id.to_owned()),
            material,
            model,
            position: scene_ref.position(),
            rotation: scene_ref.rotation(),
            tags,
        });
    }

    // Pak this asset
    pak.push_scene(key, Scene::new(refs.drain(..)))
}
