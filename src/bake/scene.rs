use crate::bake::Model;

use {
    super::{
        asset::Scene as SceneAsset, bake_material, bake_model, get_filename_key, get_path, Asset,
    },
    crate::pak::{id::SceneId, scene::Instance, PakBuf, Scene},
    std::path::{Path, PathBuf},
};

/// Reads and processes scene source files into an existing `.pak` file buffer.
pub fn bake_scene<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    filename: P2,
    asset: &SceneAsset,
    mut pak: &mut PakBuf,
) -> SceneId {
    let key = get_filename_key(&project_dir, &filename);
    if let Some(id) = pak.id(&key) {
        return id.as_scene().unwrap();
    }

    info!("Processing asset: {}", key);

    let dir = filename.as_ref().parent().unwrap();

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
            let src = get_path(&dir, src, &project_dir);
            let material = if let Some("toml") = src.extension().map(|ext| ext.to_str()).flatten() {
                Asset::read(&src).into_material().unwrap()
            } else {
                // Not sure if I want to use bitmap or a material with None PBR fields
                //Bitmap::new(&src)
                todo!();
            };
            bake_material(&project_dir, src, &material, &mut pak)
        });

        let model = scene_ref.model().map(|src| {
            let src_path = get_path(&dir, src, &project_dir);
            let model = if let Some("toml") = src.extension().map(|ext| ext.to_str()).flatten() {
                Asset::read(&src_path).into_model().unwrap()
            } else {
                Model::new(
                    PathBuf::from(&key)
                        .parent()
                        .map(|path| path.to_owned())
                        .unwrap_or_else(|| PathBuf::new())
                        .join(src),
                )
            };
            bake_model(&project_dir, src_path, &model, &mut pak)
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
