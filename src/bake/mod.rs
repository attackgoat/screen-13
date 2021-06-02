//! Contains functions and types used to bake assets into .pak files
//!
//! Assets are regular art such as `.glb`, `.jpeg` and `.ttf` files.

pub mod asset;

mod anim;
mod bitmap;
mod blob;
mod font;
mod material;
mod model;
mod scene;
mod text;

pub use self::{
    anim::bake_animation,
    asset::{Asset, Model},
    bitmap::{bake_bitmap, bake_bitmap_font},
    blob::bake_blob,
    font::bake_font,
    material::bake_material,
    model::bake_model,
    scene::bake_scene,
    text::bake_text,
};

use std::path::{Path, PathBuf};

/// Gets the fully rooted asset path from a given path. If path is relative, then dir is used to
/// determine the relative parent.
pub fn get_path<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
    path_dir: P1,
    path: P2,
    res_dir: P3,
) -> PathBuf {
    // Absolute paths are 'project aka resource directory' absolute, not *your host file system*
    // absolute!
    if path.as_ref().is_absolute() {
        // Build an array of path items (file and directories) until the root
        let mut temp = Some(path.as_ref());
        let mut parts = vec![];
        while let Some(path) = temp {
            if let Some(part) = path.file_name() {
                parts.push(part);
                temp = path.parent();
            } else {
                break;
            }
        }

        // Paste the incoming path (minus root) onto the res_dir parameter
        let mut temp = res_dir.as_ref().to_path_buf();
        for part in parts.iter().rev() {
            temp = temp.join(part);
        }

        temp.canonicalize().unwrap_or_else(|_| {
            panic!(
                "{} + {}",
                res_dir.as_ref().display(),
                path.as_ref().display()
            )
        })
    } else {
        path_dir
            .as_ref()
            .join(&path)
            .canonicalize()
            .unwrap_or_else(|_| {
                panic!(
                    "{} + {}",
                    path_dir.as_ref().display(),
                    path.as_ref().display()
                )
            })
    }
}

/// Given some parent directory and a filename, returns just the portion after the directory.
pub fn get_filename_key<P1: AsRef<Path>, P2: AsRef<Path>>(dir: P1, filename: P2) -> String {
    let res_dir = dir.as_ref();
    let mut filename = filename.as_ref();
    let mut parts = vec![];

    while filename != res_dir {
        {
            let os_filename = filename.file_name().unwrap();
            let filename_str = os_filename.to_str().unwrap();
            parts.push(filename_str.to_string());
        }
        filename = filename.parent().unwrap();
    }

    let mut key = String::new();
    for part in parts.iter().rev() {
        if !key.is_empty() {
            key.push('/');
        }

        key.push_str(part);
    }

    // Strip off the toml extension as needed
    let mut key = PathBuf::from(key);
    if let Some(ext) = key.extension() {
        if ext == "toml" {
            key = key.with_extension("");
        }
    }

    key.to_str().unwrap().to_owned()
}
