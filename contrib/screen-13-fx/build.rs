use {
    lazy_static::lazy_static,
    screen_13::prelude_all::*,
    shaderc::{Compiler, ShaderKind},
    std::{
        env::var,
        fs::{create_dir_all, write},
        path::{Path, PathBuf},
    },
};

lazy_static! {
    static ref CARGO_MANIFEST_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    static ref OUT_DIR: PathBuf = PathBuf::from(var("OUT_DIR").unwrap());
}

fn main() -> anyhow::Result<()> {
    let shader_dir = CARGO_MANIFEST_DIR.join("res/shader");
    let mut shaders = vec![];
    let glsl_shaders = glob(shader_dir.join("compute/*.comp"))?
        .into_iter()
        .chain(glob(shader_dir.join("graphic/*.vert"))?.into_iter())
        .chain(glob(shader_dir.join("graphic/*.frag"))?.into_iter())
        .map(|source_path| {
            compile_glsl(&source_path).map(|(source_code, spirv)| (source_path, source_code, spirv))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hlsl_shaders = glob(shader_dir.join("compute/*.hlsl"))?
        .into_iter()
        .map(|source_path| {
            compile_hlsl(&source_path).map(|(source_code, spirv)| (source_path, source_code, spirv))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (source_path, _source_code, spirv) in
        glsl_shaders.into_iter().chain(hlsl_shaders.into_iter())
    {
        let mut spirv_path = OUT_DIR.join(PathBuf::from(join_strings(
            remove_common_path(&shader_dir, &source_path),
            "/",
        )));
        spirv_path.set_extension(format!(
            "{}.spirv",
            source_path.extension().unwrap().to_string_lossy()
        ));
        create_dir_all(spirv_path.parent().unwrap())?;
        write(&spirv_path, &spirv)?;
        shaders.push((source_path, spirv_path));
    }

    create_shader_bindings(shader_dir, shaders, OUT_DIR.join("shader_bindings.rs"))?;

    Ok(())
}

fn join_strings(strings: impl IntoIterator<Item = String>, separator: &str) -> String {
    strings.into_iter().collect::<Vec<_>>().join(separator)
}

// Given two paths, returns the strings of the unique parts of the given path only:
// "c:\foo\bar" and "c:\foo\bar\baz\bop.txt" will return "baz\bop.txt"
fn remove_common_path(
    common_path: impl AsRef<Path>,
    path: impl AsRef<Path>,
) -> impl IntoIterator<Item = String> {
    let common_path = common_path.as_ref().to_path_buf();
    let mut path = path.as_ref().to_path_buf();
    let mut res = vec![];
    while path != common_path {
        res.push(path.file_name().unwrap().to_string_lossy().to_string());
        path = path.parent().unwrap().to_path_buf();
    }

    res.into_iter().rev()
}

fn glob(path: impl AsRef<Path>) -> anyhow::Result<impl IntoIterator<Item = PathBuf>> {
    Ok(glob::glob(path.as_ref().to_string_lossy().as_ref())?.collect::<Result<Vec<_>, _>>()?)
}

fn compile_glsl(path: impl AsRef<Path>) -> anyhow::Result<(String, Vec<u8>)> {
    let source = read_shader_source(&path);
    let mut compiler = Compiler::new().unwrap();
    let result = compiler
        .compile_into_spirv(
            &source,
            match path
                .as_ref()
                .extension()
                .map(|ext| ext.to_string_lossy().to_string())
                .unwrap_or_default()
                .as_str()
            {
                "comp" => ShaderKind::Compute,
                "frag" => ShaderKind::Fragment,
                "vert" => ShaderKind::Vertex,
                _ => unimplemented!(),
            },
            &path.as_ref().to_string_lossy(),
            "main",
            None,
        )?
        .as_binary_u8()
        .to_vec();
    Ok((source, result))
}

fn compile_hlsl(path: impl AsRef<Path>) -> anyhow::Result<(String, Vec<u8>)> {
    let source = read_shader_source(&path);
    let target_profile = if source.starts_with("#define SHADER_MODEL_cs_6_4") {
        "cs_6_4"
    } else {
        panic!("undefined target profile!");
    };
    let spirv = hassle_rs::compile_hlsl(
        path.as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .as_ref(),
        &source,
        "main",
        target_profile,
        &[
            "-spirv",
            "-enable-templates",
            //"-enable-16bit-types",
            "-fspv-target-env=vulkan1.2",
            "-WX",  // warnings as errors
            "-Ges", // strict mode
        ],
        &[],
    )?;

    Ok((source, spirv.into()))
}

fn create_shader_bindings(
    shader_path: impl AsRef<Path>,
    shaders: Vec<(PathBuf, PathBuf)>,
    dst: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let mut bindings = String::new();
    for shader in &shaders {
        bindings.push_str("pub const ");
        bindings.push_str(
            join_strings(remove_common_path(shader_path.as_ref(), &shader.0), "_")
                .to_ascii_uppercase()
                .replace('\\', "_")
                .replace('/', "_")
                .replace('-', "_")
                .replace('.', "_")
                .replace('!', "_")
                .as_str(),
        );
        bindings.push_str(": &'static [u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/");
        bindings.push_str(
            join_strings(remove_common_path(&*OUT_DIR, &shader.1), "/")
                .replace('\\', "/")
                .as_str(),
        );
        bindings.push_str("\"));\n");
    }

    write(dst, bindings)?;

    Ok(())
}

fn read_shader_source(path: impl AsRef<Path>) -> String {
    use {
        shader_prepper::{process_file, BoxedIncludeProviderError, IncludeProvider},
        std::fs::read_to_string,
    };

    struct FileIncludeProvider;

    impl IncludeProvider for FileIncludeProvider {
        type IncludeContext = PathBuf;

        fn get_include(
            &mut self,
            path: &str,
            context: &Self::IncludeContext,
        ) -> Result<(String, Self::IncludeContext), BoxedIncludeProviderError> {
            let path = context.join(path);
            println!("cargo:rerun-if-changed={}", path.display());
            Ok((
                read_to_string(&path).unwrap(),
                path.parent().unwrap().to_path_buf(),
            ))
        }
    }

    process_file(
        path.as_ref().to_string_lossy().as_ref(),
        &mut FileIncludeProvider,
        PathBuf::new(),
    )
    .unwrap()
    .iter()
    .map(|chunk| chunk.source.as_str())
    .collect()
}
