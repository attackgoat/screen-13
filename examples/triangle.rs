use {screen_13::prelude_all::*, std::iter::once};

/// In this example, we create, render and save a PBR-textured triangle in fewer than 50 lines of
/// code.
fn main() {
    let gpu = Gpu::<RcK>::offscreen();

    // Create a triangle model, which contains one mesh and three CCW POSITION/TEXCOORD vertices
    let tri = gpu
        .load_model(
            once(3),
            vec![
                (vec3(-0.5, -0.5, 0.0), vec2(0.0, 1.0)),
                (vec3(0.5, -0.5, 0.0), vec2(1.0, 1.0)),
                (vec3(0.0, 0.5, 0.0), vec2(0.5, 0.0)),
            ],
        )
        .expect(&format!(
            "{} {}",
            "Each mesh must specifiy a valid vertex count for the input list of Vertices; which",
            "must be a list of triangles.",
        ));

    // Create three 1x1 textures:
    // Color aka albedo/diffuse (RGB) -> https://en.wikipedia.org/wiki/Rust_(color)
    // Metal/Rough aka material (RG)  -> "Rusty Iron"
    // Normal map               (RGB) -> Standard pale blue 'flat'
    let color = gpu.load_bitmap(BitmapFormat::Rgb, 1, vec![0xb7, 0x41, 0x0e]);
    let metal_rough = gpu.load_bitmap(BitmapFormat::Rg, 1, vec![0xca, 0xc0]);
    let normal = gpu.load_bitmap(BitmapFormat::Rgb, 1, vec![0x00, 0x7f, 0xff]);

    // Define a pbr material
    let rust = Material {
        color,
        metal_rough,
        normal,
    };

    // Define a camera (straight lens/no perspecive)
    let dims = Extent::new(128, 128);
    let eye = Vec3::zero();
    let target = -Vec3::unit_z();
    let camera = Orthographic::new(eye, target, dims, 0.0..1.0);

    // Render + encode it to disk
    let mut render = gpu.render(dims);
    render.draw().record(
        &camera,
        &mut [
            Draw::model(tri, rust, Mat4::identity()),
            Draw::point_light(Vec3::zero(), WHITE, 1_000.0, 1.0),
        ],
    );
    render.encode().record("output.jpg");
}
