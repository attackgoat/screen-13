mod bitmap;
mod blob;
mod lang;
mod mesh;
mod pak_log;
mod scene;
mod schema;
mod text;

pub use self::{
    bitmap::{bake_bitmap, bake_font_bitmap},
    blob::bake_blob,
    lang::bake_lang,
    mesh::bake_mesh,
    pak_log::PakLog,
    scene::bake_scene,
    schema::{Asset, MeshAsset, SceneAsset},
    text::bake_text,
};

use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

// Gets the fully rooted asset path from a given path. If path is relative, then
// dir is used to determine the relative parent.
pub fn get_path<P1: AsRef<Path>, P2: AsRef<Path>>(dir: P1, path: P2) -> PathBuf {
    if path.as_ref().to_str().unwrap().starts_with('/') {
        dir.as_ref().join(PathBuf::from(
            path.as_ref().to_str().unwrap()[1..].to_owned(),
        ))
    } else {
        dir.as_ref().join(path)
    }
}

/// Given some filename and a parent directory, returns just the portion after the directory.
pub fn get_filename_key<P1: AsRef<Path>, P2: AsRef<Path>>(dir: P1, filename: P2) -> String {
    let content_dir = dir.as_ref();
    let mut filename = filename.as_ref();
    let mut parts = vec![];

    while filename != content_dir {
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
            key.push_str("/");
        }

        key.push_str(part);
    }

    key
}
