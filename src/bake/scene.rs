use {
    super::{
        asset::{Asset, AssetRef, Canonicalize as _, Material, Model, Scene},
        bake_material, bake_model, get_filename_key, is_toml,
    },
    crate::pak::{
        id::{Id, SceneId},
        scene::Instance,
        PakBuf, SceneBuf,
    },
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
    },
};

/// Reads and processes scene source files into an existing `.pak` file buffer.
pub fn bake_scene<P1, P2>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P1,
    src: P2,
    asset: &Scene,
) -> SceneId
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let key = get_filename_key(&project_dir, &src);
    if let Some(id) = context.get(&asset.clone().into()) {
        return id.as_scene().unwrap();
    }

    info!("Baking scene: {}", key);

    let src_dir = src.as_ref().parent().unwrap();

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

        let material = scene_ref
            .material()
            .map(|material| match material {
                AssetRef::Asset(material) => {
                    // Material asset specified inline
                    let mut material = material.clone();
                    material.canonicalize(&project_dir, &src_dir);
                    material
                }
                AssetRef::Path(src) => {
                    // Asset file reference
                    let src = Material::canonicalize_project_path(&project_dir, src_dir, src);
                    Asset::read(src).into_material().unwrap()
                }
            })
            .map(|material| {
                // If we do not have this model asset in the context then we must bake it
                context
                    .get(&material.clone().into())
                    .copied()
                    .unwrap_or_else(|| {
                        //bake_material(&project_dir, model.src_ref(), &model, &mut pak).into()
                        todo!()
                    })
                    .as_material()
                    .unwrap()
            });

        let model = scene_ref
            .model()
            .map(|model| match model {
                AssetRef::Asset(model) => {
                    // Model asset specified inline
                    let mut model = model.clone();
                    model.canonicalize(&project_dir, &src_dir);
                    (None, model)
                }
                AssetRef::Path(src) => {
                    let src = Model::canonicalize_project_path(&project_dir, src_dir, src);
                    if is_toml(&src) {
                        // Asset file reference
                        let mut model = Asset::read(&src).into_model().unwrap();
                        model.canonicalize(&project_dir, &src_dir);
                        (Some(src.to_owned()), model)
                    } else {
                        // Model file reference
                        (None, Model::new(src))
                    }
                }
            })
            .map(|(src, model)| bake_model(context, pak, &project_dir, src, &model));

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
    pak.push_scene(key, SceneBuf::new(refs.drain(..)))
}
