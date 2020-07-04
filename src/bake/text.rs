use {
    super::get_filename_key,
    crate::pak::PakBuf,
    std::{fs::File, io::Read, path::Path},
};

pub fn bake_text<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    pak: &mut PakBuf,
) {
    let key = get_filename_key(&project_dir, &asset_filename);

    info!(
        "Processing asset: `{}` from `{}`",
        key,
        asset_filename.as_ref().display()
    );

    let mut file = File::open(&asset_filename).unwrap();
    let mut value = String::new();
    file.read_to_string(&mut value).unwrap();

    pak.push_text(key, value);
}
