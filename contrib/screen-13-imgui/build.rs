use {
    lazy_static::lazy_static,
    shaderc::{Compiler, ShaderKind},
    std::{
        env::var,
        fs::{read_to_string, write},
        path::{Path, PathBuf},
    },
};

lazy_static! {
    static ref CARGO_MANIFEST_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    static ref OUT_DIR: PathBuf = PathBuf::from(var("OUT_DIR").unwrap());
}

fn main() -> anyhow::Result<()> {
    let shader_dir = CARGO_MANIFEST_DIR.join("res/shader");

    compile_glsl(shader_dir.join("imgui.vert"), ShaderKind::Vertex)?;
    compile_glsl(shader_dir.join("imgui.frag"), ShaderKind::Fragment)?;

    Ok(())
}

fn compile_glsl(path: impl AsRef<Path>, kind: ShaderKind) -> anyhow::Result<()> {
    let source = read_to_string(&path)?;
    let mut compiler = Compiler::new().unwrap();
    let spirv_code = compiler
        .compile_into_spirv(
            &source,
            kind,
            &path.as_ref().to_string_lossy(),
            "main",
            None,
        )?
        .as_binary_u8()
        .to_vec();

    let mut spirv_file_name = path.as_ref().to_path_buf();
    spirv_file_name.set_file_name(format!(
        "{}.spirv",
        spirv_file_name.file_name().unwrap().to_string_lossy()
    ));

    write(
        OUT_DIR.join(spirv_file_name.file_name().unwrap()),
        &spirv_code,
    )?;

    Ok(())
}
