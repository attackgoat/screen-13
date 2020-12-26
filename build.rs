#![deny(warnings)]

use {
    genmesh::{
        generators::{Cylinder, IcoSphere},
        Triangulate,
    },
    lazy_static::lazy_static,
    shaderc::{CompileOptions, Compiler, Error, ShaderKind},
    std::{
        cmp::Ordering::Equal,
        env::var,
        fs::{create_dir_all, remove_dir_all, remove_file, File},
        io::{BufRead, BufReader, Write},
        path::{Path, PathBuf},
        process::exit,
    },
};

lazy_static! {
    static ref GLSL_DIR: PathBuf = Path::new("src/gpu/glsl").to_owned();
    static ref OUT_DIR: PathBuf = Path::new(var("OUT_DIR").unwrap().as_str()).to_owned();
    static ref POINT_LIGHT_PATH: PathBuf = OUT_DIR.join("point_light.rs");
    static ref SPIRV_DIR: PathBuf = OUT_DIR.join("spirv");
    static ref SPOTLIGHT_PATH: PathBuf = OUT_DIR.join("spotlight.rs");
}

static mut GLSL_FILENAMES: Option<Vec<PathBuf>> = None;

fn main() {
    compile_shaders();
    gen_point_light();
    gen_spotlight_fn();
}

fn gen_point_light() {
    if POINT_LIGHT_PATH.exists() {
        remove_file(POINT_LIGHT_PATH.as_path()).unwrap();
    }

    let mut output_file = File::create(POINT_LIGHT_PATH.as_path()).unwrap();

    // We use a fixed-resolution icosphere for point lights, it is 320 unique vertices but we are currently
    // drawing everything as triangle lists so the total is actually 960 vertices, written out as 11,520 bytes
    let geom = IcoSphere::subdivide(2);
    let mut vertices = vec![];
    for tri in geom {
        vertices.push(tri.x.pos);
        vertices.push(tri.y.pos);
        vertices.push(tri.z.pos);
    }

    // We'll store the data as bytes because we are going to send it straight to the GPU
    writeln!(
        output_file,
        "pub const POINT_LIGHT: [u8; {}] = [",
        vertices.len() * 12
    )
    .unwrap();

    for v in vertices {
        let y = v.y.to_ne_bytes();
        let x = v.x.to_ne_bytes();
        let z = v.z.to_ne_bytes();

        writeln!(output_file, "    {}, {}, {}, {},", x[0], x[1], x[2], x[3]).unwrap();
        writeln!(output_file, "    {}, {}, {}, {},", y[0], y[1], y[2], y[3]).unwrap();
        writeln!(output_file, "    {}, {}, {}, {},", z[0], z[1], z[2], z[3]).unwrap();
    }

    writeln!(output_file, "];").unwrap();
}

/// This writes out a function that will create a spotlight at runtime without any branching code.
/// Mostly this exists because I couldn't decide how many segments spotlights should have, so you
/// can change it below to get smoother/more faceted lights.
fn gen_spotlight_fn() {
    if SPOTLIGHT_PATH.exists() {
        remove_file(SPOTLIGHT_PATH.as_path()).unwrap();
    }

    let mut output_file = File::create(SPOTLIGHT_PATH.as_path()).unwrap();

    // We use a fixed-resolution cylinder as a base geometry for spotlights
    let geom = Cylinder::new(16);
    let mut vertices = vec![];
    for tri in geom.triangulate() {
        vertices.push(tri.x.pos);
        vertices.push(tri.y.pos);
        vertices.push(tri.z.pos);
    }

    writeln!(output_file, "use std::ops::Range;\n").unwrap();
    writeln!(
        output_file,
        "pub const SPOTLIGHT_STRIDE: usize = {};\n",
        vertices.len() * 12
    )
    .unwrap();

    writeln!(
        output_file,
        r#"/// Produces the vertices of a given spotlight definition, which form a truncated cone. The resulting
/// mesh will be normalized and requires an additional scale factor to render as intended. The final location
/// will be (0,0,0) at the center of the top circle, and the orientation will point (0,-1,0).
#[allow(clippy::approx_constant)]
pub fn gen_spotlight(
    radius: Range<u8>,
    range: Range<u8>,
) -> [u8; SPOTLIGHT_STRIDE] {{
    let radius_start = radius.start as f32;
    let radius_end = radius.end as f32;
    let range_start = range.start as f32;
    let range_end = range.start as f32;

    let mut res = [0; SPOTLIGHT_STRIDE];"#
    )
    .unwrap();

    let mut radius_start_lookup = vec![];
    let mut radius_end_lookup = vec![];
    let mut range_start_lookup = vec![];
    let mut range_end_lookup = vec![];
    let search_lookup = |lookup: &mut Vec<f32>, key: f32| -> Option<usize> {
        match lookup.binary_search_by(|probe| probe.partial_cmp(&key).unwrap_or(Equal)) {
            Err(idx) => {
                lookup.insert(idx, key);
                None
            }
            Ok(idx) => Some(idx * 4),
        }
    };

    for v in vertices {
        // Swap y/z so the cylinder points to what we call down (y), also scale/translate it so the height is 1
        let x = v.x;
        let y = v.z / 2.0 - 0.5;
        let z = v.y;

        let mut dst = (radius_start_lookup.len()
            + radius_end_lookup.len()
            + range_start_lookup.len()
            + range_end_lookup.len())
            * 4;

        if x != 0.0 {
            let (lookup, part) = if y > -0.5 {
                (&mut radius_start_lookup, "radius_start")
            } else {
                (&mut radius_end_lookup, "radius_end")
            };
            if let Some(idx) = search_lookup(lookup, x) {
                writeln!(
                    output_file,
                    "    res.copy_within({}..{}, {});",
                    idx,
                    idx + 4,
                    dst
                )
                .unwrap();
            } else {
                writeln!(
                    output_file,
                    "    res[{}..{}].copy_from_slice(&({}f32 * {}).to_ne_bytes());",
                    dst,
                    dst + 4,
                    x,
                    part
                )
                .unwrap();
            }
        }

        dst += 4;

        {
            let (lookup, part) = if y > -0.5 {
                (&mut range_start_lookup, "range_start")
            } else {
                (&mut range_end_lookup, "range_end")
            };
            if let Some(idx) = search_lookup(lookup, y) {
                writeln!(
                    output_file,
                    "    res.copy_within({}..{}, {});",
                    idx,
                    idx + 4,
                    dst
                )
                .unwrap();
            } else {
                writeln!(
                    output_file,
                    "    res[{}..{}].copy_from_slice(&({}f32 + {}).to_ne_bytes());",
                    dst,
                    dst + 4,
                    y,
                    part
                )
                .unwrap();
            }
        }

        dst += 4;

        if z != 0.0 {
            let (lookup, part) = if y > -0.5 {
                (&mut radius_start_lookup, "radius_start")
            } else {
                (&mut radius_end_lookup, "radius_end")
            };
            if let Some(idx) = search_lookup(lookup, z) {
                writeln!(
                    output_file,
                    "    res.copy_within({}..{}, {});",
                    idx,
                    idx + 4,
                    dst
                )
                .unwrap();
            } else {
                writeln!(
                    output_file,
                    "    res[{}..{}].copy_from_slice(&({}f32 * {}).to_ne_bytes());",
                    dst,
                    dst + 4,
                    z,
                    part
                )
                .unwrap();
            }
        }
    }

    writeln!(output_file, "    res\n}}").unwrap();
}

fn compile_shaders() {
    unsafe {
        GLSL_FILENAMES = Some(Vec::default());
    }

    // Remove the compiled shaders directory so that we don't think things work when they don't work
    if SPIRV_DIR.exists() {
        remove_dir_all(SPIRV_DIR.as_path()).unwrap();
    }

    // Deferred rendering
    compile_glsl("defer/light.vert");
    compile_glsl("defer/line.vert");
    compile_glsl("defer/line.frag");
    compile_glsl("defer/mesh.vert");
    compile_glsl("defer/mesh.frag");
    compile_glsl("defer/point_light.frag");
    compile_glsl("defer/rect_light.frag");
    compile_glsl("defer/spotlight.frag");
    compile_glsl("defer/sunlight.frag");

    // Blending
    compile_glsl("blend/add.frag");
    compile_glsl("blend/alpha_add.frag");
    compile_glsl("blend/color.frag");
    compile_glsl("blend/color_burn.frag");
    compile_glsl("blend/color_dodge.frag");
    compile_glsl("blend/darken.frag");
    compile_glsl("blend/darker_color.frag");
    compile_glsl("blend/difference.frag");
    compile_glsl("blend/divide.frag");
    compile_glsl("blend/exclusion.frag");
    compile_glsl("blend/hard_light.frag");
    compile_glsl("blend/hard_mix.frag");
    compile_glsl("blend/linear_burn.frag");
    compile_glsl("blend/multiply.frag");
    compile_glsl("blend/normal.frag");
    compile_glsl("blend/overlay.frag");
    compile_glsl("blend/quad_transform.vert");
    compile_glsl("blend/screen.frag");
    compile_glsl("blend/subtract.frag");
    compile_glsl("blend/vivid_light.frag");

    // Compute - blurs
    compile_glsl("compute/box_blur_x.comp");
    compile_glsl("compute/box_blur_x_clamp.comp");
    compile_glsl("compute/box_blur_y.comp");
    compile_glsl("compute/box_blur_y_clamp.comp");

    // Compute - format conversion
    compile_glsl("compute/decode_rgb_rgba.comp");
    compile_glsl("compute/encode_bgr24.comp");
    compile_glsl("compute/encode_bgra32.comp");

    // Compute - General
    compile_glsl("compute/calc_vertex_attrs_u16.comp");
    compile_glsl("compute/calc_vertex_attrs_u16_skin.comp");
    compile_glsl("compute/calc_vertex_attrs_u32.comp");
    compile_glsl("compute/calc_vertex_attrs_u32_skin.comp");

    // Masking
    compile_glsl("mask/add.frag");
    compile_glsl("mask/apply.frag");
    compile_glsl("mask/darken.frag");
    compile_glsl("mask/difference.frag");
    compile_glsl("mask/draw.frag");
    compile_glsl("mask/intersect.frag");
    compile_glsl("mask/lighten.frag");
    compile_glsl("mask/subtract.frag");
    compile_glsl("mask/vertex.vert");

    // Matting
    compile_glsl("matte/alpha.frag");
    compile_glsl("matte/alpha_inv.frag");
    compile_glsl("matte/luma.frag");
    compile_glsl("matte/luma_inv.frag");

    // Skinning
    // compile_glsl("skin/anim.vert");
    // compile_glsl("skin/pose.vert");

    // Effects
    compile_glsl("brightness.frag");
    compile_glsl("clear_alpha.frag");
    compile_glsl("opacity.frag");

    // General purpose
    compile_glsl("font_outline.frag");
    compile_glsl("font.frag");
    compile_glsl("font.vert");
    compile_glsl("gradient.frag");
    compile_glsl("gradient.vert");
    compile_glsl("hdr_tonemap.frag");
    compile_glsl("post_dof.frag");
    compile_glsl("post_vignette.frag");
    compile_glsl("quad_transform.vert");
    compile_glsl("quad.vert");
    compile_glsl("skydome.frag");
    compile_glsl("skydome.vert");
    compile_glsl("shadow.frag");
    compile_glsl("shadow.vert");
    compile_glsl("ssao.frag");
    compile_glsl("texture.frag");
    compile_glsl("vertex_transform.vert");
    compile_glsl("vertex.vert");

    write_spriv_mod();
}

fn compile_glsl<P: AsRef<Path>>(filename: P) {
    let ty = match filename.as_ref().extension().unwrap().to_str().unwrap() {
        "comp" => ShaderKind::Compute,
        "frag" => ShaderKind::Fragment,
        "vert" => ShaderKind::Vertex,
        _ => panic!(),
    };

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
