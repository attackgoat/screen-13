use {
    image::io::Reader,
    screen_13::prelude::*,
    screen_13_fx::*,
    screen_13_imgui::*,
    std::{io::Cursor, time::Instant},
};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // Create Screen 13 things any similar program might need
    let event_loop = EventLoop::new()
        .window(|builder| builder.with_inner_size(LogicalSize::new(1024.0f64, 768.0f64)))
        .build()?;
    let display = ComputePresenter::new(&event_loop.device)?;
    let mut imgui = ImGui::new(&event_loop.device);
    let mut image_loader = ImageLoader::new(&event_loop.device)?;
    let mut transition_pipeline = TransitionPipeline::new(&event_loop.device);

    // Load two images for the demo to blend between
    let mut bart_image_binding = Some(
        image_loader.decode_linear(
            Reader::new(Cursor::new(include_bytes!("res/image/bart.jpg").as_slice()))
                .with_guessed_format()?
                .decode()?
                .into_rgb8()
                .to_vec()
                .as_slice(),
            ImageFormat::R8G8B8,
            1024,
            768,
        )?,
    );
    let mut gulf_image_binding = Some(
        image_loader.decode_linear(
            Reader::new(Cursor::new(include_bytes!("res/image/gulf.jpg").as_slice()))
                .with_guessed_format()?
                .decode()?
                .into_rgb8()
                .to_vec()
                .as_slice(),
            ImageFormat::R8G8B8,
            1024,
            768,
        )?,
    );

    // Hold some app state which is displayed/mutated by imgui each frame
    let mut curr_transition_idx = 0;
    let mut start_time = Instant::now();

    event_loop.run(|mut frame| {
        // Update the demo "state"
        let now = Instant::now();
        let elapsed = (now - start_time).as_secs_f32();
        let progress = if elapsed > 4.0 {
            start_time = now;
            0.0
        } else if elapsed > 3.0 {
            1.0 - (elapsed - 3.0)
        } else if elapsed > 2.0 {
            1.0
        } else if elapsed > 1.0 {
            elapsed - 1.0
        } else {
            0.0
        };

        // Bind images so we can graph them
        let bart_image = frame
            .render_graph
            .bind_node(bart_image_binding.take().unwrap());
        let gulf_image = frame
            .render_graph
            .bind_node(gulf_image_binding.take().unwrap());

        // Apply the current transition to the images and get a resultant image out; "blend_image"
        let transition = &TRANSITIONS[curr_transition_idx];
        let blend_image = transition_pipeline.apply(
            frame.render_graph,
            bart_image,
            gulf_image,
            transition,
            progress,
        );

        // Draw UI: TODO: Sliders and value setters? That would be fun.
        let gui_image = imgui.draw_frame(&mut frame, |ui| {
            imgui::Window::new("Transitions example")
                .position([10.0, 10.0], Condition::FirstUseEver)
                .size([340.0, 250.0], Condition::FirstUseEver)
                .no_decoration()
                .build(ui, || {
                    if ui.button("Next") {
                        curr_transition_idx += 1;
                        if curr_transition_idx == TRANSITIONS.len() {
                            curr_transition_idx = 0;
                        }

                        info!(
                            "{curr_transition_idx}: {:?}",
                            TRANSITIONS[curr_transition_idx]
                        );
                    }
                    ui.text_wrapped(format!("{:?}", TRANSITIONS[curr_transition_idx]));
                });
        });

        // Display the GUI + Blend images on screen
        display.present_images(
            frame.render_graph,
            gui_image,
            blend_image,
            frame.swapchain_image,
        );

        // Reclaim these bindings so we can graph them on the following frame
        bart_image_binding = Some(frame.render_graph.unbind_node(bart_image));
        gulf_image_binding = Some(frame.render_graph.unbind_node(gulf_image));
    })?;

    Ok(())
}

const TRANSITIONS: [Transition; 80] = [
    Transition::Angular {
        starting_angle: 90.0,
    },
    Transition::Bounce {
        shadow_colour: [0.0, 0.0, 0.0, 0.6],
        shadow_height: 0.075,
        bounces: 3.0,
    },
    Transition::BowTieHorizontal,
    Transition::BowTieVertical,
    Transition::BowTieWithParameter {
        adjust: 0.5,
        reverse: false,
    },
    Transition::Burn {
        color: [0.9, 0.4, 0.2],
    },
    Transition::ButterflyWaveScrawler {
        amplitude: 1.0,
        waves: 30.0,
        color_separation: 0.3,
    },
    Transition::CannabisLeaf,
    Transition::Circle {
        center: [0.5, 0.5],
        background_color: [0.1, 0.1, 0.1],
    },
    Transition::CircleCrop {
        background_color: [0.0, 0.0, 0.0, 1.0],
    },
    Transition::CircleOpen {
        smoothness: 0.3,
        opening: true,
    },
    Transition::ColorDistance { power: 5.0 },
    Transition::ColorPhase {
        from_step: [0.0, 0.2, 0.4, 0.0],
        to_step: [0.6, 0.8, 1.0, 1.0],
    },
    Transition::CoordFromIn,
    Transition::CrazyParametricFun {
        a: 4.0,
        b: 1.0,
        amplitude: 120.0,
        smoothness: 0.1,
    },
    Transition::Crosshatch {
        center: [0.5, 0.5],
        threshold: 3.0,
        fade_edge: 0.1,
    },
    Transition::CrossWarp,
    Transition::CrossZoom { strength: 0.4 },
    Transition::Cube {
        perspective: 0.7,
        unzoom: 0.3,
        reflection: 0.4,
        floating: 3.0,
    },
    Transition::Directional {
        direction: [0.0, 1.0],
    },
    Transition::DirectionalEasing {
        direction: [0.0, 1.0],
    },
    Transition::DirectionalWarp {
        direction: [-1.0, 1.0],
    },
    Transition::DirectionalWipe {
        smoothness: 0.5,
        direction: [1.0, -1.0],
    },
    // Transition::Displacement {
    //     displacement_map: AnyImageNode,
    //     strength: f32,
    // },
    Transition::DoomScreen {
        bars: 30,
        amplitude: 2.0,
        noise: 0.1,
        frequency: 0.5,
        drip_scale: 0.5,
    },
    Transition::Doorway {
        reflection: 0.4,
        perspective: 0.4,
        depth: 3.0,
    },
    Transition::Dreamy,
    Transition::DreamyZoom {
        rotation: 6.0,
        scale: 1.2,
    },
    Transition::FadeColor {
        color_phase: 4.0,
        color: [0.0, 0.0, 0.0],
    },
    Transition::Fade,
    Transition::FadeGrayscale { intensity: 0.3 },
    Transition::FilmBurn { seed: 2.31 },
    Transition::Flyeye {
        size: 0.04,
        zoom: 50.0,
        color_separation: 0.3,
    },
    Transition::GlitchDisplace,
    Transition::GlitchMemories,
    Transition::GridFlip {
        pause: 0.1,
        size: [4, 4],
        background_color: [0.0, 0.0, 0.0, 1.0],
        divider_width: 0.05,
        randomness: 0.1,
    },
    Transition::Heart,
    Transition::Hexagonalize {
        steps: 50,
        horizontal_hexagons: 20.0,
    },
    Transition::InvertedPageCurl,
    Transition::Kaleidoscope {
        speed: 1.0,
        angle: 1.0,
        power: 1.5,
    },
    Transition::LeftRight,
    Transition::LinearBlur { intensity: 0.1 },
    // Transition::Luma {
    //     luma_map: AnyImageNode,
    // },
    Transition::LuminanceMelt {
        direction: true,
        threshold: 0.8,
        above: false,
    },
    Transition::Morph { strength: 0.1 },
    Transition::Mosaic { end: [2, -1] },
    Transition::Multiply,
    Transition::Overexposure { strength: 0.6 },
    Transition::Perlin {
        scale: 4.0,
        smoothness: 0.01,
        seed: 12.9898,
    },
    Transition::Pinwheel { speed: 2.0 },
    Transition::Pixelize {
        steps: 50,
        squares_min: [20, 20],
    },
    Transition::PolarFunction { segments: 5 },
    Transition::PolkaDotsCurtain {
        dots: 20.0,
        center: [0.0, 0.0],
    },
    Transition::PowerKaleido {
        scale: 2.0,
        z: 1.5,
        speed: 5.0,
    },
    Transition::Radial { smoothness: 1.0 },
    Transition::RandomNoisex,
    Transition::RandomSquares {
        smoothness: 0.5,
        size: [10, 10],
    },
    Transition::Ripple {
        amplitude: 100.0,
        speed: 50.0,
    },
    Transition::Rotate,
    Transition::RotateScale {
        rotations: 1.0,
        center: [0.5, 0.5],
        background_color: [0.15, 0.15, 0.15, 1.0],
        scale: 8.0,
    },
    Transition::ScaleIn,
    Transition::SimpleZoom {
        zoom_quickness: 0.8,
    },
    Transition::SquaresWire {
        smoothness: 1.6,
        squares: [10, 10],
        direction: [1.0, -0.5],
    },
    Transition::Squeeze {
        color_separation: 0.04,
    },
    Transition::StereoViewer {
        zoom: 0.88,
        corner_radius: 0.22,
    },
    Transition::Swap {
        reflection: 0.4,
        perspective: 0.2,
        depth: 3.0,
    },
    Transition::Swirl,
    Transition::TangentMotionBlur,
    Transition::TopBottom,
    Transition::TvStatic { offset: 0.05 },
    Transition::UndulatingBurnOut {
        smoothness: 0.03,
        center: [0.5, 0.5],
        color: [0.0, 0.0, 0.0],
    },
    Transition::WaterDrop {
        amplitude: 30.0,
        speed: 30.0,
    },
    Transition::Wind { size: 0.2 },
    Transition::WindowBlinds,
    Transition::WindowSlice {
        count: 10.0,
        smoothness: 0.5,
    },
    Transition::WipeDown,
    Transition::WipeLeft,
    Transition::WipeRight,
    Transition::WipeUp,
    Transition::ZoomInCircles,
    Transition::ZoomLeftWipe {
        zoom_quickness: 0.8,
    },
    Transition::ZoomRightWipe {
        zoom_quickness: 0.8,
    },
];
