use {screen_13::prelude::*, screen_13_hot::prelude::*, std::path::PathBuf};

/// This program draws a noise signal to the swapchain - make changes to fill_image.hlsl or the
/// noise.hlsl file it includes to see those changes update while the program is still running.
///
/// Run with RUST_LOG=info to get notification of shader compilations.
fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let event_loop = EventLoop::new().build()?;

    // Create a graphic pipeline - the same as normal except for "Hot" prefixes and we provide the
    // shader source code path instead of the shader source code bytes
    let cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fill_image_path = cargo_manifest_dir.join("examples/res/fill_image.hlsl");
    let mut pipeline = HotGraphicPipeline::create(
        &event_loop.device,
        GraphicPipelineInfo::default(),
        [
            HotShader::new_vertex(&fill_image_path).entry_name("vertex_main".to_string()),
            HotShader::new_fragment(&fill_image_path).entry_name("fragment_main".to_string()),
        ],
    )?;

    let mut frame_index: u32 = 0;

    event_loop.run(|frame| {
        frame
            .render_graph
            .begin_pass("make some noise")
            .bind_pipeline(pipeline.hot())
            .clear_color(0, frame.swapchain_image)
            .store_color(0, frame.swapchain_image)
            .record_subpass(move |subpass, _| {
                subpass
                    .push_constants_offset(0, &frame_index.to_ne_bytes())
                    .push_constants_offset(4, &frame.width.to_ne_bytes())
                    .push_constants_offset(8, &frame.height.to_ne_bytes())
                    .draw(3, 1, 0, 0);
            });

        frame_index += 1;
    })
}
