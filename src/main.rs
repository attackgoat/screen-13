//#![deny(warnings)]
#![allow(dead_code)]

#[macro_use]
extern crate log;

mod bake;
mod color;
mod math;
mod pak;

use {
    self::{
        bake::{
            asset::{Asset, Bitmap, Canonicalize as _, Model},
            bake_animation, bake_bitmap, bake_bitmap_font, bake_blob, bake_material, bake_model,
            bake_scene, parent,
        },
        pak::PakBuf,
    },
    pretty_env_logger::init,
    std::{
        collections::HashMap,
        env::{args, current_dir, current_exe},
        fs::{create_dir_all, File},
        io::{BufWriter, Error as IoError},
        path::PathBuf,
    },
};

fn main() -> Result<(), IoError> {
    // Enable logging
    init();

    // What to bake (the input text file)
    let project_arg = args().nth(1).unwrap_or_else(|| {
        panic!(
            "{} {}",
            "No project specified; re-run this command with the name of a project file as the",
            "argument. Example: `cargo run foo_program.toml`",
        )
    });

    // Where to put the baked .pak
    // TODO: This needs to be easier to use when not running demos; it should work with relative
    // paths (canonicalized) and such for building external projects!
    let pak_arg = args().nth(2).unwrap_or_else(|| {
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    });

    // Input project .toml file
    let project_path = current_dir()
        .unwrap()
        .join(&project_arg)
        .canonicalize()
        .unwrap();
    let project_dir = project_path.parent().unwrap();

    // Output .pak file
    let pak_dir = PathBuf::from(&pak_arg);
    let pak_path = pak_dir
        .join(project_path.file_name().unwrap())
        .with_extension("pak");

    debug!("Baking project `{}`", project_path.display());
    debug!("Output pak `{}`", pak_path.display());

    // Create the output directory as needed
    if !pak_dir.exists() {
        create_dir_all(&pak_dir).unwrap();
    }

    // We use a hashing log thingy to keep track of which assets may have already been baked
    // This helps when multiple models or scenes reference the same items. The entire asset
    // instance is hashed because we cannot count on the source filepath to be the same as
    // it may be a relative path.
    let mut pak = PakBuf::default();

    // TODO: Find a home:
    // Bake::Blob => bake_blob(&project_dir, &asset_filename, &mut pak),
    // Bake::Text => bake_text(&project_dir, &asset_filename, &mut pak),

    let mut context = HashMap::new();

    // Process each file we find
    let content = Asset::read(&project_path).into_content().unwrap();
    for group in content.groups() {
        if !group.enabled() {
            continue;
        }

        for asset in group.assets() {
            let src = project_dir.join(asset);

            //debug!("Asset: {}", asset_filename.display());
            let src_dir = parent(project_dir, &src);

            match src
                .extension()
                .map(|ext| ext.to_str())
                .flatten()
                .map(|ext| ext.to_lowercase())
                .as_deref()
                .unwrap_or_else(|| panic!("Unexpected extensionless file {}", src.display()))
            {
                "otf" | "ttc" | "ttf" => bake_blob(&mut pak, project_dir, src),
                "glb" | "gltf" => {
                    // Note that direct references like this build a model, not an animation
                    // To build an animation you must specify a .toml file
                    let mut model = Model::new(&src);
                    model.canonicalize(project_dir, src_dir);
                    bake_model(&mut context, &mut pak, project_dir, Some(src), &model);
                }
                "jpg" | "jpeg" | "png" | "bmp" | "tga" | "dds" | "webp" | "gif" | "ico"
                | "tiff" => {
                    let mut bitmap = Bitmap::new(&src);
                    bitmap.canonicalize(project_dir, src_dir);
                    bake_bitmap(&mut context, &mut pak, &project_dir, Some(src), &bitmap);
                }
                "toml" => match Asset::read(&src) {
                    Asset::Animation(anim) => {
                        // bake_animation(&mut context, &project_dir, asset_filename, anim, &mut pak);
                        todo!();
                    }
                    // Asset::Atlas(ref atlas) => {
                    //     bake_atlas(&project_dir, &asset_filename, atlas, &mut pak);
                    // }
                    Asset::Bitmap(bitmap) => {
                        bake_bitmap(&mut context, &mut pak, &project_dir, Some(src), &bitmap);
                    }
                    Asset::BitmapFont(bitmap_font) => {
                        bake_bitmap_font(&mut context, &mut pak, project_dir, src, bitmap_font);
                    }
                    Asset::Color(_) => unreachable!(),
                    Asset::Content(_) => {
                        // Nested content files are not yet supported
                        panic!("Unexpected content file {}", src.display());
                    }
                    // Asset::Language(ref lang) => {
                    //     bake_lang(&project_dir, &asset_filename, lang, &mut pak, &mut log)
                    // }
                    Asset::Material(material) => {
                        bake_material(&mut context, &mut pak, project_dir, Some(src), &material);
                    }
                    Asset::Model(model) => {
                        bake_model(&mut context, &mut pak, project_dir, Some(src), &model);
                    }
                    Asset::Scene(scene) => {
                        bake_scene(&mut context, &mut pak, &project_dir, src, &scene);
                    }
                },
                ext => unimplemented!("Unexpected file extension {}", ext),
            }
        }
    }

    // Write the output pak file
    debug!("Writing pak");

    pak.write(
        &mut BufWriter::new(File::create(&pak_path).unwrap()),
        content.compression(),
    )?;

    debug!("Baked project successfully");

    Ok(())
}
