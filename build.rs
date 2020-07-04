#![deny(warnings)]

use {
    lazy_static::lazy_static,
    shaderc::{CompileOptions, Compiler, Error, ShaderKind},
    std::{
        env::var,
        fs::{create_dir_all, remove_dir_all, File},
        io::{BufRead, BufReader, Write},
        path::{Path, PathBuf},
        process::exit,
    },
};

lazy_static! {
    static ref GLSL_DIR: PathBuf = Path::new("src/gpu/glsl").to_owned();
    static ref OUT_DIR: PathBuf = Path::new(var("OUT_DIR").unwrap().as_str()).to_owned();
    static ref SPIRV_DIR: PathBuf = OUT_DIR.join("spirv");
}

static mut GLSL_FILENAMES: Option<Vec<PathBuf>> = None;

fn main() {
    unsafe {
        GLSL_FILENAMES = Some(Vec::default());
    }

    // Remove the compiled shaders directory so that we don't think things work when they don't work
    if SPIRV_DIR.exists() {
        remove_dir_all(SPIRV_DIR.as_path()).unwrap();
    }

    // Deferred renderings
    compile_glsl(ShaderKind::Fragment, "deferred/mesh_dual.frag");
    compile_glsl(ShaderKind::Vertex, "deferred/mesh_dual.vert");
    compile_glsl(ShaderKind::Fragment, "deferred/mesh_single.frag");
    compile_glsl(ShaderKind::Vertex, "deferred/mesh_single.vert");
    compile_glsl(ShaderKind::Fragment, "deferred/trans.frag");
    compile_glsl(ShaderKind::Fragment, "deferred/spotlight.frag");
    compile_glsl(ShaderKind::Fragment, "deferred/sunlight.frag");

    // Blending
    compile_glsl(ShaderKind::Fragment, "blending/add.frag");
    compile_glsl(ShaderKind::Fragment, "blending/alpha_add.frag");
    compile_glsl(ShaderKind::Fragment, "blending/color.frag");
    compile_glsl(ShaderKind::Fragment, "blending/color_burn.frag");
    compile_glsl(ShaderKind::Fragment, "blending/color_dodge.frag");
    compile_glsl(ShaderKind::Fragment, "blending/darken.frag");
    compile_glsl(ShaderKind::Fragment, "blending/darker_color.frag");
    compile_glsl(ShaderKind::Fragment, "blending/difference.frag");
    compile_glsl(ShaderKind::Fragment, "blending/divide.frag");
    compile_glsl(ShaderKind::Fragment, "blending/exclusion.frag");
    compile_glsl(ShaderKind::Fragment, "blending/hard_light.frag");
    compile_glsl(ShaderKind::Fragment, "blending/hard_mix.frag");
    compile_glsl(ShaderKind::Fragment, "blending/linear_burn.frag");
    compile_glsl(ShaderKind::Fragment, "blending/multiply.frag");
    compile_glsl(ShaderKind::Fragment, "blending/normal.frag");
    compile_glsl(ShaderKind::Fragment, "blending/overlay.frag");
    compile_glsl(ShaderKind::Vertex, "blending/quad_transform.vert");
    compile_glsl(ShaderKind::Fragment, "blending/screen.frag");
    compile_glsl(ShaderKind::Fragment, "blending/subtract.frag");
    compile_glsl(ShaderKind::Fragment, "blending/vivid_light.frag");

    // Compute - blurs
    compile_glsl(ShaderKind::Compute, "compute/box_blur_x.comp");
    compile_glsl(ShaderKind::Compute, "compute/box_blur_x_clamp.comp");
    compile_glsl(ShaderKind::Compute, "compute/box_blur_y.comp");
    compile_glsl(ShaderKind::Compute, "compute/box_blur_y_clamp.comp");

    // Compute - RGB/RGBA
    compile_glsl(ShaderKind::Compute, "compute/decode_bgr24.comp");
    compile_glsl(ShaderKind::Compute, "compute/decode_bgra32.comp");
    compile_glsl(ShaderKind::Compute, "compute/encode_bgr24.comp");
    compile_glsl(ShaderKind::Compute, "compute/encode_bgra32.comp");

    // Masking
    compile_glsl(ShaderKind::Fragment, "masking/add.frag");
    compile_glsl(ShaderKind::Fragment, "masking/apply.frag");
    compile_glsl(ShaderKind::Fragment, "masking/darken.frag");
    compile_glsl(ShaderKind::Fragment, "masking/difference.frag");
    compile_glsl(ShaderKind::Fragment, "masking/draw.frag");
    compile_glsl(ShaderKind::Fragment, "masking/intersect.frag");
    compile_glsl(ShaderKind::Fragment, "masking/lighten.frag");
    compile_glsl(ShaderKind::Fragment, "masking/subtract.frag");
    compile_glsl(ShaderKind::Vertex, "masking/vertex.vert");

    // Matting
    compile_glsl(ShaderKind::Fragment, "matting/alpha.frag");
    compile_glsl(ShaderKind::Fragment, "matting/alpha_inverted.frag");
    compile_glsl(ShaderKind::Fragment, "matting/luma.frag");
    compile_glsl(ShaderKind::Fragment, "matting/luma_inverted.frag");

    // Effects
    compile_glsl(ShaderKind::Fragment, "brightness.frag");
    compile_glsl(ShaderKind::Fragment, "clear_alpha.frag");
    compile_glsl(ShaderKind::Fragment, "opacity.frag");

    // General purpose
    compile_glsl(ShaderKind::Fragment, "font_outline.frag");
    compile_glsl(ShaderKind::Fragment, "font.frag");
    compile_glsl(ShaderKind::Vertex, "font.vert");
    compile_glsl(ShaderKind::Fragment, "gradient.frag");
    compile_glsl(ShaderKind::Vertex, "gradient.vert");
    compile_glsl(ShaderKind::Fragment, "hdr_tonemap.frag");
    compile_glsl(ShaderKind::Vertex, "line.vert");
    compile_glsl(ShaderKind::Fragment, "line.frag");
    compile_glsl(ShaderKind::Fragment, "post_dof.frag");
    compile_glsl(ShaderKind::Fragment, "post_vignette.frag");
    compile_glsl(ShaderKind::Vertex, "quad_transform.vert");
    compile_glsl(ShaderKind::Vertex, "quad.vert");
    compile_glsl(ShaderKind::Fragment, "shadow.frag");
    compile_glsl(ShaderKind::Vertex, "shadow.vert");
    compile_glsl(ShaderKind::Fragment, "ssao.frag");
    compile_glsl(ShaderKind::Fragment, "texture.frag");
    compile_glsl(ShaderKind::Vertex, "vertex_transform.vert");
    compile_glsl(ShaderKind::Vertex, "vertex.vert");

    write_spriv_mod();
}

fn compile_glsl<P: AsRef<Path>>(ty: ShaderKind, filename: P) {
    // Read the source code
    let glsl = read_file_with_includes(GLSL_DIR.join(&filename));

    let filename = filename.as_ref().to_owned();

    unsafe {
        GLSL_FILENAMES.as_mut().unwrap().push(filename.clone());
    }

    // Compile the source code or print out help
    let mut spirv = match compile_spirv(&glsl, ty, filename.to_str().unwrap()) {
        Ok(spirv) => spirv,
        Err(err) => {
            // Print the file that failed
            eprintln!("Compile failed: {}", filename.to_str().unwrap());

            // Print each line so we can see what the expansion looked like
            let mut line_num = 1;
            for line in glsl.lines() {
                eprintln!("{}: {}", line_num, line);
                line_num += 1;
            }

            eprintln!("{}", err);

            exit(1);
        }
    };

    // Create the output directory and file
    let filename = SPIRV_DIR.join(
        &filename
            .with_file_name(format!(
                "{}_{}",
                filename.file_stem().unwrap().to_str().unwrap(),
                filename
                    .extension()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(".", "_")
            ))
            .with_extension("rs"),
    );
    create_dir_all(filename.parent().unwrap()).unwrap();
    let mut output_file = File::create(&filename).unwrap();

    //
    let mut spirv_wide = vec![];
    while !spirv.is_empty() {
        let mut byte_wide = 0u32;
        if spirv.len() >= 4 {
            byte_wide |= (spirv.remove(3) as u32) << 24;
        }
        if spirv.len() >= 3 {
            byte_wide |= (spirv.remove(2) as u32) << 16;
        }
        if spirv.len() >= 2 {
            byte_wide |= (spirv.remove(1) as u32) << 8;
        }
        if !spirv.is_empty() {
            byte_wide |= spirv.remove(0) as u32;
        }
        spirv_wide.push(byte_wide);
    }

    // Convert to a byte array string
    let bytes = spirv_wide
        .iter()
        .map(|&val| format!("0x{:x}", val))
        .collect::<Vec<String>>()
        .join(", ");

    // Write a maybe-okay helper function
    write!(
        output_file,
        "#[allow(clippy::all)]\npub const SPIRV: [u32; {}] = [{}];",
        spirv_wide.len(),
        bytes
    )
    .unwrap();
}

fn compile_spirv(glsl: &str, ty: ShaderKind, filename: &str) -> Result<Vec<u8>, Error> {
    let mut compiler = Compiler::new().unwrap();
    let mut options = CompileOptions::new().unwrap();
    options.add_macro_definition("EP", Some("main"));
    let result = compiler
        .compile_into_spirv(glsl, ty, filename, "main", Some(&options))?
        .as_binary_u8()
        .to_vec();
    Ok(result)
}

fn read_file_with_includes<P: AsRef<Path>>(filename: P) -> String {
    println!(
        "cargo:rerun-if-changed={}",
        filename.as_ref().to_str().unwrap()
    );

    let mut result = String::new();
    let mut reader = BufReader::new(File::open(&filename).unwrap());
    let mut line = String::new();

    // Read each line in the file
    while 0 < reader.read_line(&mut line).unwrap() {
        // Remove the trailing newline char
        if line.ends_with('\n') {
            let len = line.len();
            line.truncate(len - 1);
        }

        // If the line is an include tag, recursively include it
        if line.starts_with("#include \"") && line.ends_with('"') {
            // Remove leading tag and quote
            line.drain(0..10);

            // Remove trailing quote
            let len = line.len();
            line.truncate(len - 1);

            // Bring in the contents of the include file
            let include_filename = filename.as_ref().parent().unwrap().join(line.clone()); // TODO: Should probably do this so the changes to relative files work "ie ../folder/thing": `.canonicalize().unwrap();`
            line = read_file_with_includes(include_filename);

            // Remove the trailing newline char
            if line.ends_with('\n') {
                let len = line.len();
                line.truncate(len - 1);
            }
        }

        // Add this line (or lines) to the result
        result.push_str(&line);
        result.push('\n');
        line.clear();
    }

    result
}

/// Note: This doesn't support multi-level folders, such as assets\complicated\shader.vert
///
/// This only supports one level of folder
fn write_spriv_mod() {
    let mut directories = Vec::default();

    unsafe {
        GLSL_FILENAMES.as_mut().unwrap().sort();
    }

    // Make sure each directory has its own mod
    unsafe {
        for filename in GLSL_FILENAMES.as_ref().unwrap() {
            // Does filename have a preceding path portion? ex: assets\shader.frag
            if filename.file_name().unwrap() != filename.as_os_str() {
                let parent = filename.parent().unwrap();
                if !directories.contains(&parent) {
                    write_spriv_mod_at(&parent);
                    directories.push(parent);
                }
            }
        }
    }

    write_spriv_mod_at("");
}

fn write_spriv_mod_at<P: AsRef<Path>>(path: P) {
    let filename = SPIRV_DIR.join(&path).join("mod").with_extension("rs");
    let mut output_file = File::create(&filename).unwrap();
    let path = path.as_ref().as_os_str();
    let mut filenames = Vec::default();
    let mut directories = Vec::default();

    // Get the filenames for `path`
    unsafe {
        for filename in GLSL_FILENAMES.as_ref().unwrap() {
            // Does filename have a preceding path portion? ex: assets\shader.frag
            if filename.file_name().unwrap() != filename.as_os_str() {
                let parent = filename.parent().unwrap();
                if parent.as_os_str() == path {
                    filenames.push(filename.clone());
                }

                if path.is_empty() && !directories.contains(&parent) {
                    directories.push(parent);
                }
            } else if path.is_empty() {
                filenames.push(filename.clone());
            }
        }
    }

    for directory in &directories {
        writeln!(output_file, "pub mod {};", directory.display()).unwrap();
    }

    writeln!(output_file).unwrap();

    for filename in &filenames {
        writeln!(
            output_file,
            "mod {}_{};",
            filename.file_stem().unwrap().to_str().unwrap(),
            filename.extension().unwrap().to_str().unwrap()
        )
        .unwrap();
    }

    writeln!(output_file).unwrap();

    for filename in &filenames {
        writeln!(
            output_file,
            "pub use self::{}_{}::SPIRV as {}_{};",
            filename.file_stem().unwrap().to_str().unwrap(),
            filename.extension().unwrap().to_str().unwrap(),
            filename
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_uppercase(),
            filename
                .extension()
                .unwrap()
                .to_str()
                .unwrap()
                .to_uppercase(),
        )
        .unwrap();
    }
}
