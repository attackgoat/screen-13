//#![deny(warnings)]
#![allow(dead_code)] // TODO: Remove at some point

#[macro_use]
extern crate log;

mod bake;
mod math;
mod pak;

use {
    self::{
        bake::{
            bake_animation, bake_bitmap, bake_bitmap_font, bake_font, bake_material, bake_model,
            bake_scene, Asset,
        },
        pak::PakBuf,
    },
    pretty_env_logger::init,
    std::{
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

    // Process each file we find
    let content = Asset::read(&project_path).into_content().unwrap();
    for group in content.groups() {
        if group.enabled() {
            for asset in group.assets() {
                let asset_filename = project_dir.join(asset);

                match Asset::read(&asset_filename) {
                    Asset::Animation(ref anim) => {
                        bake_animation(&project_dir, asset_filename, anim, &mut pak);
                    }
                    // Asset::Atlas(ref atlas) => {
                    //     bake_atlas(&project_dir, &asset_filename, atlas, &mut pak);
                    // }
                    Asset::Bitmap(ref bitmap) => {
                        bake_bitmap(&project_dir, &asset_filename, bitmap, &mut pak);
                    }
                    Asset::BitmapFont(ref bitmap_font) => {
                        bake_bitmap_font(&project_dir, &asset_filename, bitmap_font, &mut pak);
                    }
                    Asset::Font(font) => {
                        bake_font(&project_dir, &asset_filename, &font, &mut pak);
                    }
                    // Asset::Language(ref lang) => {
                    //     bake_lang(&project_dir, &asset_filename, lang, &mut pak, &mut log)
                    // }
                    Asset::Material(ref material) => {
                        bake_material(&project_dir, &asset_filename, material, &mut pak);
                    }
                    Asset::Model(ref model) => {
                        bake_model(&project_dir, &asset_filename, model, &mut pak);
                    }
                    Asset::Scene(scene) => {
                        bake_scene(&project_dir, &asset_filename, &scene, &mut pak);
                    }
                    _ => panic!(),
                }
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
