use screen_13::gpu::Gpu;

fn main() {
    // Create a 128x128 pixel render
    let gpu = Gpu::offscreen();
    let mut render = gpu.render((128u32, 128u32));

    // Clear with black
    render.clear().record();

    // Save as jpeg
    render.encode().record("output.jpg");
}
