//#![deny(warnings)]
#![allow(dead_code)]

#[macro_use]
extern crate log;

mod bake;
mod math;
mod pak;

use {
    self::{
        bake::{
            bake_animation, bake_bitmap, bake_blob, bake_font_bitmap, bake_material, bake_model,
            bake_scene, bake_text, Asset, PakLog,
        },
        pak::PakBuf,
    },
    pretty_env_logger::init,
    std::{
        env::{args, current_dir, current_exe},
        fs::{create_dir_all, File},
        io::{BufRead, BufReader, BufWriter, Error as IoError},
        path::PathBuf,
    },
};

// #[cfg(debug_assertions)]
// use engine::{init_debug, log::debug};

fn main() -> Result<(), IoError> {
    // Enable logging
    init();

    // What to bake (the input text file)
    let project_arg = args().nth(1).expect("No project specified; re-run this command with the name of a project file as the argument. Example: `cargo run my_game.s13`");

    // Where to put the baked .pak
    // TODO: This needs to be easier to use when not running demos; it should work with relative paths (canonicalized) and such for building external projects!
    let pak_arg = args().nth(2).unwrap_or_else(|| {
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    });

    // Input project text file
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
    let mut log = PakLog::default();

    // Process each file we find
    for line in BufReader::new(File::open(&project_path).unwrap()).lines() {
        let mut line = line.unwrap();

        // Figure out which type of thing we're baking
        let bake = if line.trim_start().starts_with('#') || line.trim_start().is_empty() {
            // Nothing - this was a comment or blank
            continue;
        } else if line.starts_with("BLOB ") {
            // Item is a raw byte array
            Bake::Blob
        } else if line.starts_with("TEXT ") {
            // Item is a huge string
            Bake::Text
        } else {
            // Item is a .toml file of some type
            Bake::Asset
        };

        // Strip the leading text from basic asset types (BLOB or TEXT)
        match bake {
            Bake::Blob | Bake::Text => line = line.split_off(5),
            _ => (),
        }

        let mut assets = vec![line];
        while let Some(asset) = assets.pop() {
            let asset_filename = project_dir.join(PathBuf::from(&asset));

            match bake {
                Bake::Asset => match Asset::read(&asset_filename) {
                    Asset::Animation(ref anim) => {
                        bake_animation(&project_dir, asset_filename, anim, &mut pak, &mut log);
                    }
                    // Asset::Atlas(ref atlas) => {
                    //     bake_atlas(&project_dir, &asset_filename, atlas, &mut pak, &mut log);
                    // }
                    Asset::Bitmap(ref bitmap) => {
                        bake_bitmap(&project_dir, &asset_filename, bitmap, &mut pak, &mut log);
                    }
                    Asset::FontBitmap(ref bitmap) => {
                        bake_font_bitmap(&project_dir, &asset_filename, bitmap, &mut pak, &mut log);
                    }
                    // Asset::Language(ref lang) => {
                    //     bake_lang(&project_dir, &asset_filename, lang, &mut pak, &mut log)
                    // }
                    Asset::Material(ref material) => {
                        bake_material(&project_dir, &asset_filename, material, &mut pak, &mut log);
                    }
                    Asset::Model(ref model) => {
                        bake_model(&project_dir, &asset_filename, model, &mut pak, &mut log);
                    }
                    Asset::Scene(scene) => {
                        bake_scene(&project_dir, &asset_filename, &scene, &mut pak, &mut log);
                    }
                },
                Bake::Blob => bake_blob(&project_dir, &asset_filename, &mut pak),
                Bake::Text => bake_text(&project_dir, &asset_filename, &mut pak),
            }
        }
    }

    // Write the output pak file
    debug!("Writing pak");

    pak.write(
        &mut BufWriter::new(File::create(&pak_path).unwrap()),
        Default::default(),
    )?;

    debug!("Baked project successfully");

    Ok(())
}

enum Bake {
    Asset,
    Blob,
    Text,
}

// enum CookError {
//     Content(String),
//     Io(IoError),
//     Pak(Box<PakErrorKind>),
// }

// impl<'a> From<&'a str> for CookError {
//     fn from(value: &str) -> CookError {
//         CookError::Content(value.to_owned())
//     }
// }

// impl From<IoError> for CookError {
//     fn from(value: IoError) -> CookError {
//         CookError::Io(value)
//     }
// }

// impl From<Box<PakErrorKind>> for CookError {
//     fn from(value: Box<PakErrorKind>) -> CookError {
//         CookError::Pak(value)
//     }
// }
