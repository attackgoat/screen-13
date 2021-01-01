use {screen_13::prelude_all::*, std::iter::once};

fn main() {
    let gpu = Gpu::offscreen();

    // Create a triangle model, which contains one mesh and three CCW POSITION/TEXCOORD vertices
    let tri = gpu.load_model(
        once(3),
        vec![
            (vec3(-0.5, -0.5, 0.0), vec2(0.0, 1.0)),
            (vec3(0.5, -0.5, 0.0), vec2(1.0, 1.0)),
            (vec3(0.0, 0.5, 0.0), vec2(0.5, 0.0)),
        ],
    ).expect("Each mesh must specifiy a valid vertex count for the input list of Vertices; which must be a list of triangles.");

    // Wrap our triangle Model with a shared reference (required so we can draw it)
    let tri = ModelRef::new(tri);

    // Render + encode it to disk
    let dims = Extent::new(128, 128);
    let eye = Vec3::zero();
    let target = -Vec3::unit_z();
    let camera = Orthographic::new(eye, target, dims, 0.0..1.0);
    let render = gpu.render(dims);
    //render.draw().record(&camera, &mut [Draw::model(tri,  Mat4::identity())]);
}
