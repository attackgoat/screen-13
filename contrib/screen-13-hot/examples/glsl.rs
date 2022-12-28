use {
    lazy_static::lazy_static, screen_13::prelude::*, screen_13_hot::prelude::*, std::path::PathBuf,
};

lazy_static! {
    static ref CARGO_MANIFEST_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
}

/// This program draws a noise signal to the swapchain - make changes to fill_image.comp or the
/// noise.glsl file it includes to see those changes update while the program is still running.
///
/// Run with RUST_LOG=info to get notification of shader compilations.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new().build()?;

    // Create a compute pipeline - the same as normal except for "Hot" prefixes and we provide the
    // shader source code path instead of the shader source code bytes
    let mut pipeline = HotComputePipeline::create(
        &event_loop.device,
        ComputePipelineInfo::default(),
        HotShader::new_compute(CARGO_MANIFEST_DIR.join("examples/res/fill_image.comp")),
    )?;

    let mut frame_index: u32 = 0;

    event_loop.run(|frame| {
        frame
            .render_graph
            .begin_pass("make some noise")
            .bind_pipeline(pipeline.hot())
            .write_descriptor(0, frame.swapchain_image)
            .record_compute(move |compute, _| {
                compute.push_constants(&frame_index.to_ne_bytes()).dispatch(
                    frame.width,
                    frame.height,
                    1,
                );
            });

        frame_index += 1;
    })
}
