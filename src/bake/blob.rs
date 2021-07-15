use {
    super::get_filename_key,
    crate::pak::PakBuf,
    std::{fs::File, io::Read, path::Path},
};

/// Reads and processes arbitrary binary source files into an existing `.pak` file buffer.
pub fn bake_blob<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    pak: &mut PakBuf,
) {
    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    let mut file = File::open(asset_filename).unwrap();
    let mut value = vec![];
    file.read_to_end(&mut value).unwrap();

    pak.push_blob(key, value);
}
