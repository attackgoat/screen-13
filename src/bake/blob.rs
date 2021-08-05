use {
    super::get_filename_key,
    crate::pak::PakBuf,
    std::{fs::File, io::Read, path::Path},
};

/// Reads and processes arbitrary binary source files into an existing `.pak` file buffer.
pub fn bake_blob<P1, P2>(pak: &mut PakBuf, project_dir: P1, src: P2)
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let key = get_filename_key(&project_dir, &src);

    info!("Baking blob: {}", key);

    let mut file = File::open(src).unwrap();
    let mut value = vec![];
    file.read_to_end(&mut value).unwrap();

    pak.push_blob(Some(key), value);
}
