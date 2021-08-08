//! Contains functions and types used to bake assets into .pak files
//!
//! Assets are regular art such as `.glb`, `.jpeg` and `.ttf` files.

pub mod asset;

mod anim;
mod bitmap;
mod blob;
mod material;
mod model;
mod scene;
mod text;

pub use self::{
    anim::bake_animation,
    bitmap::{bake_bitmap, bake_bitmap_font},
    blob::bake_blob,
    material::bake_material,
    model::bake_model,
    scene::bake_scene,
    text::bake_text,
};

use std::path::{Path, PathBuf};

// TODO: TEST!
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
    if is_toml(&key) {
        key = key.with_extension("");
    }

    key.to_str().unwrap().to_owned()
}

/// Returns either the parent directory of the given path or the project root if the path has no
/// parent.
pub fn parent<P1, P2>(project_dir: P1, path: P2) -> PathBuf
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    path.as_ref()
        .parent()
        .map(|path| path.to_owned())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Returns `true` when a given path has the `.toml` file extension.
fn is_toml<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    path.as_ref()
        .extension()
        .map(|ext| ext.to_str())
        .flatten()
        .filter(|ext| *ext == "toml")
        .is_some()
}
