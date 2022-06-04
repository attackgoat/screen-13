// Adapted from https://github.com/gl-transitions/gl-transitions
// NOTE: Some are rough or broken and need a bit of care - others should be optimized for production
// use.

use {
    inline_spirv::include_spirv,
    screen_13::prelude::*,
    std::{collections::HashMap, sync::Arc},
};

#[derive(Clone, Debug)]
pub enum Transition {
    Angular {
        starting_angle: f32,
    },
    Bounce {
        shadow_height: f32,
        bounces: f32,
        shadow_colour: [f32; 4],
    },
    BowTieHorizontal,
    BowTieVertical,
    BowTieWithParameter {
        adjust: f32,
        reverse: bool,
    },
    Burn {
        color: [f32; 3],
    },
    ButterflyWaveScrawler {
        amplitude: f32,
        waves: f32,
        color_separation: f32,
    },
    CannabisLeaf,
    Circle {
        center: [f32; 2],
        background_color: [f32; 3],
    },
    CircleCrop {
        background_color: [f32; 4],
    },
    CircleOpen {
        smoothness: f32,
        opening: bool,
    },
    ColorDistance {
        power: f32,
    },
    ColorPhase {
        from_step: [f32; 4],
        to_step: [f32; 4],
    },
    CoordFromIn,
    CrazyParametricFun {
        a: f32,
        b: f32,
        amplitude: f32,
        smoothness: f32,
    },
    Crosshatch {
        center: [f32; 2],
        threshold: f32,
        fade_edge: f32,
    },
    CrossWarp,
    CrossZoom {
        strength: f32,
    },
    Cube {
        perspective: f32,
        unzoom: f32,
        reflection: f32,
        floating: f32,
    },
    Directional {
        direction: [f32; 2],
    },
    DirectionalEasing {
        direction: [f32; 2],
    },
    DirectionalWarp {
        direction: [f32; 2],
    },
    DirectionalWipe {
        smoothness: f32,
        direction: [f32; 2],
    },
    Displacement {
        displacement_map: AnyImageNode,
        strength: f32,
    },
    DoomScreen {
        /// Number of total bars/columns
        bars: i32,

        /// Multiplier for speed ratio. 0 = no variation when going down, higher = some elements go much faster
        amplitude: f32,

        /// Further variations in speed. 0 = no noise, 1 = super noisy (ignore frequency)
        noise: f32,

        /// Speed variation horizontally. the bigger the value, the shorter the waves
        frequency: f32,

        /// How much the bars seem to "run" from the middle of the screen first (sticking to the sides). 0 = no drip, 1 = curved drip
        drip_scale: f32,
    },
    Doorway {
        reflection: f32,
        perspective: f32,
        depth: f32,
    },
    Dreamy,
    DreamyZoom {
        /// In degrees
        rotation: f32,

        /// Multiplier
        scale: f32,
    },
    FadeColor {
        /// if 0.0, there is no black phase, if 0.9, the black phase is very important
        color_phase: f32,

        color: [f32; 3],
    },
    Fade,
    FadeGrayscale {
        /// if 0.0, the image directly turn grayscale, if 0.9, the grayscale transition phase is very important
        intensity: f32,
    },
    FilmBurn {
        seed: f32,
    },
    Flyeye {
        size: f32,
        zoom: f32,
        color_separation: f32,
    },
    GlitchDisplace,
    GlitchMemories,
    GridFlip {
        pause: f32,
        size: [i32; 2],
        background_color: [f32; 4],
        divider_width: f32,
        randomness: f32,
    },
    Heart,
    Hexagonalize {
        steps: i32,
        horizontal_hexagons: f32,
    },
    InvertedPageCurl,
    Kaleidoscope {
        speed: f32,
        angle: f32,
        power: f32,
    },
    LeftRight,
    LinearBlur {
        intensity: f32,
    },
    Luma {
        luma_map: AnyImageNode,
    },
    LuminanceMelt {
        /// Direction of movement :  0 : up, 1, down
        direction: bool,

        /// Luminance threshold
        threshold: f32,

        /// Does the movement takes effect above or below luminance threshold ?
        above: bool,
    },
    Morph {
        strength: f32,
    },
    Mosaic {
        end: [i32; 2],
    },
    Multiply,
    Overexposure {
        strength: f32,
    },
    Perlin {
        scale: f32,
        smoothness: f32,
        seed: f32,
    },
    Pinwheel {
        speed: f32,
    },
    Pixelize {
        /// Zero disables stepping
        steps: i32,

        /// Minimum number of squares (when the effect is at its higher level)
        squares_min: [i32; 2],
    },
    PolarFunction {
        segments: i32,
    },
    PolkaDotsCurtain {
        dots: f32,
        center: [f32; 2],
    },
    PowerKaleido {
        scale: f32,
        z: f32,
        speed: f32,
    },
    Radial {
        smoothness: f32,
    },
    RandomNoisex,
    RandomSquares {
        smoothness: f32,
        size: [i32; 2],
    },
    Ripple {
        amplitude: f32,
        speed: f32,
    },
    Rotate,
    RotateScale {
        rotations: f32,
        center: [f32; 2],
        background_color: [f32; 4],
        scale: f32,
    },
    ScaleIn,
    SimpleZoom {
        zoom_quickness: f32,
    },
    SquaresWire {
        smoothness: f32,
        squares: [i32; 2],
        direction: [f32; 2],
    },
    Squeeze {
        color_separation: f32,
    },
    StereoViewer {
        /// How much to zoom (out) for the effect ~ 0.5 - 1.0
        zoom: f32,

        /// Corner radius as a fraction of the image height
        corner_radius: f32,
    },
    Swap {
        reflection: f32,
        perspective: f32,
        depth: f32,
    },
    Swirl,
    TangentMotionBlur,
    TopBottom,
    TvStatic {
        offset: f32,
    },
    UndulatingBurnOut {
        smoothness: f32,
        center: [f32; 2],
        color: [f32; 3],
    },
    WaterDrop {
        amplitude: f32,
        speed: f32,
    },
    Wind {
        size: f32,
    },
    WindowBlinds,
    WindowSlice {
        count: f32,
        smoothness: f32,
    },
    WipeDown,
    WipeLeft,
    WipeRight,
    WipeUp,
    ZoomInCircles,
    ZoomLeftWipe {
        zoom_quickness: f32,
    },
    ZoomRightWipe {
        zoom_quickness: f32,
    },
}

impl Transition {
    fn ty(&self) -> TransitionType {
        match self {
            Self::Angular { .. } => TransitionType::Angular,
            Self::Bounce { .. } => TransitionType::Bounce,
            Self::BowTieHorizontal => TransitionType::BowTieHorizontal,
            Self::BowTieVertical => TransitionType::BowTieVertical,
            Self::BowTieWithParameter { .. } => TransitionType::BowTieWithParameter,
            Self::Burn { .. } => TransitionType::Burn,
            Self::ButterflyWaveScrawler { .. } => TransitionType::ButterflyWaveScrawler,
            Self::CannabisLeaf { .. } => TransitionType::CannabisLeaf,
            Self::Circle { .. } => TransitionType::Circle,
            Self::CircleCrop { .. } => TransitionType::CircleCrop,
            Self::CircleOpen { .. } => TransitionType::CircleOpen,
            Self::ColorDistance { .. } => TransitionType::ColorDistance,
            Self::ColorPhase { .. } => TransitionType::ColorPhase,
            Self::CoordFromIn => TransitionType::CoordFromIn,
            Self::CrazyParametricFun { .. } => TransitionType::CrazyParametricFun,
            Self::Crosshatch { .. } => TransitionType::Crosshatch,
            Self::CrossWarp => TransitionType::CrossWarp,
            Self::CrossZoom { .. } => TransitionType::CrossZoom,
            Self::Cube { .. } => TransitionType::Cube,
            Self::Directional { .. } => TransitionType::Directional,
            Self::DirectionalEasing { .. } => TransitionType::DirectionalEasing,
            Self::DirectionalWarp { .. } => TransitionType::DirectionalWarp,
            Self::DirectionalWipe { .. } => TransitionType::DirectionalWipe,
            Self::Displacement { .. } => TransitionType::Displacement,
            Self::DoomScreen { .. } => TransitionType::DoomScreen,
            Self::Doorway { .. } => TransitionType::Doorway,
            Self::Dreamy => TransitionType::Dreamy,
            Self::DreamyZoom { .. } => TransitionType::DreamyZoom,
            Self::FadeColor { .. } => TransitionType::FadeColor,
            Self::Fade => TransitionType::Fade,
            Self::FadeGrayscale { .. } => TransitionType::FadeGrayscale,
            Self::FilmBurn { .. } => TransitionType::FilmBurn,
            Self::Flyeye { .. } => TransitionType::Flyeye,
            Self::GlitchDisplace => TransitionType::GlitchDisplace,
            Self::GlitchMemories => TransitionType::GlitchMemories,
            Self::GridFlip { .. } => TransitionType::GridFlip,
            Self::Heart => TransitionType::Heart,
            Self::Hexagonalize { .. } => TransitionType::Hexagonalize,
            Self::InvertedPageCurl => TransitionType::InvertedPageCurl,
            Self::Kaleidoscope { .. } => TransitionType::Kaleidoscope,
            Self::LeftRight => TransitionType::LeftRight,
            Self::LinearBlur { .. } => TransitionType::LinearBlur,
            Self::Luma { .. } => TransitionType::Luma,
            Self::LuminanceMelt { .. } => TransitionType::LuminanceMelt,
            Self::Morph { .. } => TransitionType::Morph,
            Self::Mosaic { .. } => TransitionType::Mosaic,
            Self::Multiply => TransitionType::Multiply,
            Self::Overexposure { .. } => TransitionType::Overexposure,
            Self::Perlin { .. } => TransitionType::Perlin,
            Self::Pinwheel { .. } => TransitionType::Pinwheel,
            Self::Pixelize { .. } => TransitionType::Pixelize,
            Self::PolarFunction { .. } => TransitionType::PolarFunction,
            Self::PolkaDotsCurtain { .. } => TransitionType::PolkaDotsCurtain,
            Self::PowerKaleido { .. } => TransitionType::PowerKaleido,
            Self::Radial { .. } => TransitionType::Radial,
            Self::RandomNoisex => TransitionType::RandomNoisex,
            Self::RandomSquares { .. } => TransitionType::RandomSquares,
            Self::Ripple { .. } => TransitionType::Ripple,
            Self::Rotate => TransitionType::Rotate,
            Self::RotateScale { .. } => TransitionType::RotateScale,
            Self::ScaleIn => TransitionType::ScaleIn,
            Self::SimpleZoom { .. } => TransitionType::SimpleZoom,
            Self::SquaresWire { .. } => TransitionType::SquaresWire,
            Self::Squeeze { .. } => TransitionType::Squeeze,
            Self::StereoViewer { .. } => TransitionType::StereoViewer,
            Self::Swap { .. } => TransitionType::Swap,
            Self::Swirl => TransitionType::Swirl,
            Self::TangentMotionBlur => TransitionType::TangentMotionBlur,
            Self::TopBottom => TransitionType::TopBottom,
            Self::TvStatic { .. } => TransitionType::TvStatic,
            Self::UndulatingBurnOut { .. } => TransitionType::UndulatingBurnOut,
            Self::WaterDrop { .. } => TransitionType::WaterDrop,
            Self::Wind { .. } => TransitionType::Wind,
            Self::WindowBlinds => TransitionType::WindowBlinds,
            Self::WindowSlice { .. } => TransitionType::WindowSlice,
            Self::WipeDown => TransitionType::WipeDown,
            Self::WipeLeft => TransitionType::WipeLeft,
            Self::WipeRight => TransitionType::WipeRight,
            Self::WipeUp => TransitionType::WipeUp,
            Self::ZoomInCircles => TransitionType::ZoomInCircles,
            Self::ZoomLeftWipe { .. } => TransitionType::ZoomLeftWipe,
            Self::ZoomRightWipe { .. } => TransitionType::ZoomRightWipe,
        }
    }
}

pub struct TransitionPipeline {
    cache: HashPool,
    pipelines: HashMap<TransitionType, Arc<ComputePipeline>>,
}

impl TransitionPipeline {
    pub fn new(device: &Arc<Device>) -> Self {
        let cache = HashPool::new(device);
        let pipelines = Default::default();

        Self { cache, pipelines }
    }

    pub fn apply(
        &mut self,
        render_graph: &mut RenderGraph,
        a_image: impl Into<AnyImageNode>,
        b_image: impl Into<AnyImageNode>,
        transition: &Transition,
        progress: f32,
    ) -> ImageLeaseNode {
        let a_image = a_image.into();
        let b_image = b_image.into();
        let progress = progress.max(0.0).min(1.0);

        let a_info = render_graph.node_info(a_image);
        let b_info = render_graph.node_info(b_image);

        let dest_info = ImageInfo::new_2d(
            vk::Format::R8G8B8A8_UNORM,
            a_info.width.max(b_info.width),
            a_info.height.max(b_info.height),
            vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::TRANSFER_SRC,
        )
        .build();
        let dest_image = render_graph.bind_node(self.cache.lease(dest_info).unwrap());

        // Lazy-initialize the compute pipeline for this transition
        let transition_ty = transition.ty();
        let pipeline = self.pipelines.entry(transition_ty).or_insert_with(|| {
            trace!("creating {transition_ty:?}");

            Arc::new(
                ComputePipeline::create(
                    &self.cache.device,
                    match transition_ty {
                        TransitionType::Angular => {
                            include_spirv!("res/shader/transition/angular.comp", comp).as_slice()
                        }
                        TransitionType::Bounce => {
                            include_spirv!("res/shader/transition/bounce.comp", comp).as_slice()
                        }
                        TransitionType::BowTieHorizontal => {
                            include_spirv!("res/shader/transition/bow_tie_horizontal.comp", comp)
                                .as_slice()
                        }
                        TransitionType::BowTieVertical => {
                            include_spirv!("res/shader/transition/bow_tie_vertical.comp", comp)
                                .as_slice()
                        }
                        TransitionType::BowTieWithParameter => include_spirv!(
                            "res/shader/transition/bow_tie_with_parameter.comp",
                            comp
                        )
                        .as_slice(),
                        TransitionType::Burn => {
                            include_spirv!("res/shader/transition/burn.comp", comp).as_slice()
                        }
                        TransitionType::ButterflyWaveScrawler => include_spirv!(
                            "res/shader/transition/butterfly_wave_scrawler.comp",
                            comp
                        )
                        .as_slice(),
                        TransitionType::CannabisLeaf => {
                            include_spirv!("res/shader/transition/cannabis_leaf.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Circle => {
                            include_spirv!("res/shader/transition/circle.comp", comp).as_slice()
                        }
                        TransitionType::CircleCrop => {
                            include_spirv!("res/shader/transition/circle_crop.comp", comp)
                                .as_slice()
                        }
                        TransitionType::CircleOpen => {
                            include_spirv!("res/shader/transition/circle_open.comp", comp)
                                .as_slice()
                        }
                        TransitionType::ColorDistance => {
                            include_spirv!("res/shader/transition/color_distance.comp", comp)
                                .as_slice()
                        }
                        TransitionType::ColorPhase => {
                            include_spirv!("res/shader/transition/color_phase.comp", comp)
                                .as_slice()
                        }
                        TransitionType::CoordFromIn => {
                            include_spirv!("res/shader/transition/coord_from_in.comp", comp)
                                .as_slice()
                        }
                        TransitionType::CrazyParametricFun => {
                            include_spirv!("res/shader/transition/crazy_parametric_fun.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Crosshatch => {
                            include_spirv!("res/shader/transition/crosshatch.comp", comp).as_slice()
                        }
                        TransitionType::CrossWarp => {
                            include_spirv!("res/shader/transition/cross_warp.comp", comp).as_slice()
                        }
                        TransitionType::CrossZoom => {
                            include_spirv!("res/shader/transition/cross_zoom.comp", comp).as_slice()
                        }
                        TransitionType::Cube => {
                            include_spirv!("res/shader/transition/cube.comp", comp).as_slice()
                        }
                        TransitionType::Directional => {
                            include_spirv!("res/shader/transition/directional.comp", comp)
                                .as_slice()
                        }
                        TransitionType::DirectionalEasing => {
                            include_spirv!("res/shader/transition/directional_easing.comp", comp)
                                .as_slice()
                        }
                        TransitionType::DirectionalWarp => {
                            include_spirv!("res/shader/transition/directional_warp.comp", comp)
                                .as_slice()
                        }
                        TransitionType::DirectionalWipe => {
                            include_spirv!("res/shader/transition/directional_wipe.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Displacement => {
                            include_spirv!("res/shader/transition/displacement.comp", comp)
                                .as_slice()
                        }
                        TransitionType::DoomScreen => {
                            include_spirv!("res/shader/transition/doom_screen.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Doorway => {
                            include_spirv!("res/shader/transition/doorway.comp", comp).as_slice()
                        }
                        TransitionType::Dreamy => {
                            include_spirv!("res/shader/transition/dreamy.comp", comp).as_slice()
                        }
                        TransitionType::DreamyZoom => {
                            include_spirv!("res/shader/transition/dreamy_zoom.comp", comp)
                                .as_slice()
                        }
                        TransitionType::FadeColor => {
                            include_spirv!("res/shader/transition/fade_color.comp", comp).as_slice()
                        }
                        TransitionType::Fade => {
                            include_spirv!("res/shader/transition/fade.comp", comp).as_slice()
                        }
                        TransitionType::FadeGrayscale => {
                            include_spirv!("res/shader/transition/fade_grayscale.comp", comp)
                                .as_slice()
                        }
                        TransitionType::FilmBurn => {
                            include_spirv!("res/shader/transition/film_burn.comp", comp).as_slice()
                        }
                        TransitionType::Flyeye => {
                            include_spirv!("res/shader/transition/flyeye.comp", comp).as_slice()
                        }
                        TransitionType::GlitchDisplace => {
                            include_spirv!("res/shader/transition/glitch_displace.comp", comp)
                                .as_slice()
                        }
                        TransitionType::GlitchMemories => {
                            include_spirv!("res/shader/transition/glitch_memories.comp", comp)
                                .as_slice()
                        }
                        TransitionType::GridFlip => {
                            include_spirv!("res/shader/transition/grid_flip.comp", comp).as_slice()
                        }
                        TransitionType::Heart => {
                            include_spirv!("res/shader/transition/heart.comp", comp).as_slice()
                        }
                        TransitionType::Hexagonalize => {
                            include_spirv!("res/shader/transition/hexagonalize.comp", comp)
                                .as_slice()
                        }
                        TransitionType::InvertedPageCurl => {
                            include_spirv!("res/shader/transition/inverted_page_curl.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Kaleidoscope => {
                            include_spirv!("res/shader/transition/kaleidoscope.comp", comp)
                                .as_slice()
                        }
                        TransitionType::LeftRight => {
                            include_spirv!("res/shader/transition/left_right.comp", comp).as_slice()
                        }
                        TransitionType::LinearBlur => {
                            include_spirv!("res/shader/transition/linear_blur.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Luma => {
                            include_spirv!("res/shader/transition/luma.comp", comp).as_slice()
                        }
                        TransitionType::LuminanceMelt => {
                            include_spirv!("res/shader/transition/luminance_melt.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Morph => {
                            include_spirv!("res/shader/transition/morph.comp", comp).as_slice()
                        }
                        TransitionType::Mosaic => {
                            include_spirv!("res/shader/transition/mosaic.comp", comp).as_slice()
                        }
                        TransitionType::Multiply => {
                            include_spirv!("res/shader/transition/multiply.comp", comp).as_slice()
                        }
                        TransitionType::Overexposure => {
                            include_spirv!("res/shader/transition/overexposure.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Perlin => {
                            include_spirv!("res/shader/transition/perlin.comp", comp).as_slice()
                        }
                        TransitionType::Pinwheel => {
                            include_spirv!("res/shader/transition/pinwheel.comp", comp).as_slice()
                        }
                        TransitionType::Pixelize => {
                            include_spirv!("res/shader/transition/pixelize.comp", comp).as_slice()
                        }
                        TransitionType::PolarFunction => {
                            include_spirv!("res/shader/transition/polar_function.comp", comp)
                                .as_slice()
                        }
                        TransitionType::PolkaDotsCurtain => {
                            include_spirv!("res/shader/transition/polka_dots_curtain.comp", comp)
                                .as_slice()
                        }
                        TransitionType::PowerKaleido => {
                            include_spirv!("res/shader/transition/power_kaleido.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Radial => {
                            include_spirv!("res/shader/transition/radial.comp", comp).as_slice()
                        }
                        TransitionType::RandomNoisex => {
                            include_spirv!("res/shader/transition/random_noisex.comp", comp)
                                .as_slice()
                        }
                        TransitionType::RandomSquares => {
                            include_spirv!("res/shader/transition/random_squares.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Ripple => {
                            include_spirv!("res/shader/transition/ripple.comp", comp).as_slice()
                        }
                        TransitionType::Rotate => {
                            include_spirv!("res/shader/transition/rotate.comp", comp).as_slice()
                        }
                        TransitionType::RotateScale => {
                            include_spirv!("res/shader/transition/rotate_scale.comp", comp)
                                .as_slice()
                        }
                        TransitionType::ScaleIn => {
                            include_spirv!("res/shader/transition/scale_in.comp", comp).as_slice()
                        }
                        TransitionType::SimpleZoom => {
                            include_spirv!("res/shader/transition/simple_zoom.comp", comp)
                                .as_slice()
                        }
                        TransitionType::SquaresWire => {
                            include_spirv!("res/shader/transition/squares_wire.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Squeeze => {
                            include_spirv!("res/shader/transition/squeeze.comp", comp).as_slice()
                        }
                        TransitionType::StereoViewer => {
                            include_spirv!("res/shader/transition/stereo_viewer.comp", comp)
                                .as_slice()
                        }
                        TransitionType::Swap => {
                            include_spirv!("res/shader/transition/swap.comp", comp).as_slice()
                        }
                        TransitionType::Swirl => {
                            include_spirv!("res/shader/transition/swirl.comp", comp).as_slice()
                        }
                        TransitionType::TangentMotionBlur => {
                            include_spirv!("res/shader/transition/tangent_motion_blur.comp", comp)
                                .as_slice()
                        }
                        TransitionType::TopBottom => {
                            include_spirv!("res/shader/transition/top_bottom.comp", comp).as_slice()
                        }
                        TransitionType::TvStatic => {
                            include_spirv!("res/shader/transition/tv_static.comp", comp).as_slice()
                        }
                        TransitionType::UndulatingBurnOut => {
                            include_spirv!("res/shader/transition/undulating_burn_out.comp", comp)
                                .as_slice()
                        }
                        TransitionType::WaterDrop => {
                            include_spirv!("res/shader/transition/water_drop.comp", comp).as_slice()
                        }
                        TransitionType::Wind => {
                            include_spirv!("res/shader/transition/wind.comp", comp).as_slice()
                        }
                        TransitionType::WindowBlinds => {
                            include_spirv!("res/shader/transition/window_blinds.comp", comp)
                                .as_slice()
                        }
                        TransitionType::WindowSlice => {
                            include_spirv!("res/shader/transition/window_slice.comp", comp)
                                .as_slice()
                        }
                        TransitionType::WipeDown => {
                            include_spirv!("res/shader/transition/wipe_down.comp", comp).as_slice()
                        }
                        TransitionType::WipeLeft => {
                            include_spirv!("res/shader/transition/wipe_left.comp", comp).as_slice()
                        }
                        TransitionType::WipeRight => {
                            include_spirv!("res/shader/transition/wipe_right.comp", comp).as_slice()
                        }
                        TransitionType::WipeUp => {
                            include_spirv!("res/shader/transition/wipe_up.comp", comp).as_slice()
                        }
                        TransitionType::ZoomInCircles => {
                            include_spirv!("res/shader/transition/zoom_in_circles.comp", comp)
                                .as_slice()
                        }
                        TransitionType::ZoomLeftWipe => {
                            include_spirv!("res/shader/transition/zoom_left_wipe.comp", comp)
                                .as_slice()
                        }
                        TransitionType::ZoomRightWipe => {
                            include_spirv!("res/shader/transition/zoom_right_wipe.comp", comp)
                                .as_slice()
                        }
                    },
                )
                .unwrap(),
            )
        });

        let mut push_consts = Vec::with_capacity(128);
        push_consts.extend_from_slice(&progress.to_ne_bytes());

        match transition {
            &Transition::Angular { starting_angle } => {
                push_consts.extend_from_slice(&starting_angle.to_ne_bytes());
            }
            &Transition::Bounce {
                shadow_height,
                bounces,
                shadow_colour,
            } => {
                push_consts.extend_from_slice(&shadow_height.to_ne_bytes());
                push_consts.extend_from_slice(&bounces.to_ne_bytes());
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&shadow_colour[0].to_ne_bytes());
                push_consts.extend_from_slice(&shadow_colour[1].to_ne_bytes());
                push_consts.extend_from_slice(&shadow_colour[2].to_ne_bytes());
                push_consts.extend_from_slice(&shadow_colour[3].to_ne_bytes());
            }
            &Transition::BowTieWithParameter { adjust, reverse } => {
                push_consts.extend_from_slice(&adjust.to_ne_bytes());
                push_consts.extend_from_slice(&(reverse as u32).to_ne_bytes());
            }
            &Transition::Burn { color } => {
                push_consts.extend_from_slice(&[0u8; 12]); // padding
                push_consts.extend_from_slice(&color[0].to_ne_bytes());
                push_consts.extend_from_slice(&color[1].to_ne_bytes());
                push_consts.extend_from_slice(&color[2].to_ne_bytes());
            }
            &Transition::ButterflyWaveScrawler {
                amplitude,
                waves,
                color_separation,
            } => {
                push_consts.extend_from_slice(&amplitude.to_ne_bytes());
                push_consts.extend_from_slice(&waves.to_ne_bytes());
                push_consts.extend_from_slice(&color_separation.to_ne_bytes());
            }
            &Transition::Circle {
                center,
                background_color,
            } => {
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&center[0].to_ne_bytes());
                push_consts.extend_from_slice(&center[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[0].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[2].to_ne_bytes());
                push_consts.extend_from_slice(&[0u8; 4]); // padding
            }
            &Transition::CircleCrop { background_color } => {
                push_consts.extend_from_slice(&[0u8; 12]); // padding
                push_consts.extend_from_slice(&background_color[0].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[2].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[3].to_ne_bytes());
                push_consts.extend_from_slice(&[0u8; 4]); // padding
            }
            &Transition::CircleOpen {
                smoothness,
                opening,
            } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&(opening as u32).to_ne_bytes());
            }
            &Transition::ColorDistance { power } => {
                push_consts.extend_from_slice(&power.to_ne_bytes());
            }
            &Transition::ColorPhase { from_step, to_step } => {
                push_consts.extend_from_slice(&[0u8; 12]); // padding
                push_consts.extend_from_slice(&from_step[0].to_ne_bytes());
                push_consts.extend_from_slice(&from_step[1].to_ne_bytes());
                push_consts.extend_from_slice(&from_step[2].to_ne_bytes());
                push_consts.extend_from_slice(&from_step[3].to_ne_bytes());
                push_consts.extend_from_slice(&to_step[0].to_ne_bytes());
                push_consts.extend_from_slice(&to_step[1].to_ne_bytes());
                push_consts.extend_from_slice(&to_step[2].to_ne_bytes());
                push_consts.extend_from_slice(&to_step[3].to_ne_bytes());
            }
            &Transition::CrazyParametricFun {
                a,
                b,
                amplitude,
                smoothness,
            } => {
                push_consts.extend_from_slice(&a.to_ne_bytes());
                push_consts.extend_from_slice(&b.to_ne_bytes());
                push_consts.extend_from_slice(&amplitude.to_ne_bytes());
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
            }
            &Transition::Crosshatch {
                center,
                threshold,
                fade_edge,
            } => {
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&center[0].to_ne_bytes());
                push_consts.extend_from_slice(&center[1].to_ne_bytes());
                push_consts.extend_from_slice(&threshold.to_ne_bytes());
                push_consts.extend_from_slice(&fade_edge.to_ne_bytes());
            }
            &Transition::CrossZoom { strength } => {
                push_consts.extend_from_slice(&strength.to_ne_bytes());
            }
            &Transition::Cube {
                perspective,
                unzoom,
                reflection,
                floating,
            } => {
                push_consts.extend_from_slice(&perspective.to_ne_bytes());
                push_consts.extend_from_slice(&unzoom.to_ne_bytes());
                push_consts.extend_from_slice(&reflection.to_ne_bytes());
                push_consts.extend_from_slice(&floating.to_ne_bytes());
            }
            &Transition::Directional { direction } => {
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&direction[0].to_ne_bytes());
                push_consts.extend_from_slice(&direction[1].to_ne_bytes());
            }
            &Transition::DirectionalEasing { direction } => {
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&direction[0].to_ne_bytes());
                push_consts.extend_from_slice(&direction[1].to_ne_bytes());
            }
            &Transition::DirectionalWarp { direction } => {
                push_consts.extend_from_slice(&[0u8; 4]); // padding
                push_consts.extend_from_slice(&direction[0].to_ne_bytes());
                push_consts.extend_from_slice(&direction[1].to_ne_bytes());
            }
            &Transition::DirectionalWipe {
                smoothness,
                direction,
            } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&direction[0].to_ne_bytes());
                push_consts.extend_from_slice(&direction[1].to_ne_bytes());
            }
            &Transition::Displacement { strength, .. } => {
                push_consts.extend_from_slice(&strength.to_ne_bytes());
            }
            &Transition::DoomScreen {
                bars,
                amplitude,
                noise,
                frequency,
                drip_scale,
            } => {
                push_consts.extend_from_slice(&bars.to_ne_bytes());
                push_consts.extend_from_slice(&amplitude.to_ne_bytes());
                push_consts.extend_from_slice(&noise.to_ne_bytes());
                push_consts.extend_from_slice(&frequency.to_ne_bytes());
                push_consts.extend_from_slice(&drip_scale.to_ne_bytes());
            }
            &Transition::Doorway {
                reflection,
                perspective,
                depth,
            } => {
                push_consts.extend_from_slice(&reflection.to_ne_bytes());
                push_consts.extend_from_slice(&perspective.to_ne_bytes());
                push_consts.extend_from_slice(&depth.to_ne_bytes());
            }
            &Transition::DreamyZoom { rotation, scale } => {
                push_consts.extend_from_slice(&rotation.to_ne_bytes());
                push_consts.extend_from_slice(&scale.to_ne_bytes());
            }
            &Transition::FadeColor { color_phase, color } => {
                push_consts.extend_from_slice(&color_phase.to_ne_bytes());
                push_consts.extend_from_slice(&[0u8; 8]); // padding
                push_consts.extend_from_slice(&color[0].to_ne_bytes());
                push_consts.extend_from_slice(&color[1].to_ne_bytes());
                push_consts.extend_from_slice(&color[2].to_ne_bytes());
            }
            &Transition::FadeGrayscale { intensity } => {
                push_consts.extend_from_slice(&intensity.to_ne_bytes());
            }
            &Transition::FilmBurn { seed } => {
                push_consts.extend_from_slice(&seed.to_ne_bytes());
            }
            &Transition::Flyeye {
                size,
                zoom,
                color_separation,
            } => {
                push_consts.extend_from_slice(&size.to_ne_bytes());
                push_consts.extend_from_slice(&zoom.to_ne_bytes());
                push_consts.extend_from_slice(&color_separation.to_ne_bytes());
            }
            &Transition::GridFlip {
                pause,
                size,
                background_color,
                divider_width,
                randomness,
            } => {
                push_consts.extend_from_slice(&pause.to_ne_bytes());
                push_consts.extend_from_slice(&size[0].to_ne_bytes());
                push_consts.extend_from_slice(&size[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[0].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[2].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[3].to_ne_bytes());
                push_consts.extend_from_slice(&divider_width.to_ne_bytes());
                push_consts.extend_from_slice(&randomness.to_ne_bytes());
            }
            &Transition::Hexagonalize {
                steps,
                horizontal_hexagons,
            } => {
                push_consts.extend_from_slice(&steps.to_ne_bytes());
                push_consts.extend_from_slice(&horizontal_hexagons.to_ne_bytes());
            }
            &Transition::Kaleidoscope {
                speed,
                angle,
                power,
            } => {
                push_consts.extend_from_slice(&speed.to_ne_bytes());
                push_consts.extend_from_slice(&angle.to_ne_bytes());
                push_consts.extend_from_slice(&power.to_ne_bytes());
            }
            &Transition::LinearBlur { intensity } => {
                push_consts.extend_from_slice(&intensity.to_ne_bytes());
            }
            &Transition::LuminanceMelt {
                direction,
                threshold,
                above,
            } => {
                push_consts.extend_from_slice(&(direction as u32).to_ne_bytes());
                push_consts.extend_from_slice(&threshold.to_ne_bytes());
                push_consts.extend_from_slice(&(above as u32).to_ne_bytes());
            }
            &Transition::Morph { strength } => {
                push_consts.extend_from_slice(&strength.to_ne_bytes());
            }
            &Transition::Mosaic { end } => {
                push_consts.extend_from_slice(&end[0].to_ne_bytes());
                push_consts.extend_from_slice(&end[1].to_ne_bytes());
            }
            &Transition::Overexposure { strength } => {
                push_consts.extend_from_slice(&strength.to_ne_bytes());
            }
            &Transition::Perlin {
                scale,
                smoothness,
                seed,
            } => {
                push_consts.extend_from_slice(&scale.to_ne_bytes());
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&seed.to_ne_bytes());
            }
            &Transition::Pinwheel { speed } => {
                push_consts.extend_from_slice(&speed.to_ne_bytes());
            }
            &Transition::Pixelize { steps, squares_min } => {
                push_consts.extend_from_slice(&steps.to_ne_bytes());
                push_consts.extend_from_slice(&squares_min[0].to_ne_bytes());
                push_consts.extend_from_slice(&squares_min[1].to_ne_bytes());
            }
            &Transition::PolarFunction { segments } => {
                push_consts.extend_from_slice(&segments.to_ne_bytes());
            }
            &Transition::PolkaDotsCurtain { dots, center } => {
                push_consts.extend_from_slice(&dots.to_ne_bytes());
                push_consts.extend_from_slice(&center[0].to_ne_bytes());
                push_consts.extend_from_slice(&center[1].to_ne_bytes());
            }
            &Transition::PowerKaleido { scale, z, speed } => {
                push_consts.extend_from_slice(&scale.to_ne_bytes());
                push_consts.extend_from_slice(&z.to_ne_bytes());
                push_consts.extend_from_slice(&speed.to_ne_bytes());
            }
            &Transition::Radial { smoothness } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
            }
            &Transition::RandomSquares { smoothness, size } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&size[0].to_ne_bytes());
                push_consts.extend_from_slice(&size[1].to_ne_bytes());
            }
            &Transition::Ripple { amplitude, speed } => {
                push_consts.extend_from_slice(&amplitude.to_ne_bytes());
                push_consts.extend_from_slice(&speed.to_ne_bytes());
            }
            &Transition::RotateScale {
                rotations,
                center,
                background_color,
                scale,
            } => {
                push_consts.extend_from_slice(&rotations.to_ne_bytes());
                push_consts.extend_from_slice(&center[0].to_ne_bytes());
                push_consts.extend_from_slice(&center[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[0].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[1].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[2].to_ne_bytes());
                push_consts.extend_from_slice(&background_color[3].to_ne_bytes());
                push_consts.extend_from_slice(&scale.to_ne_bytes());
            }
            &Transition::SimpleZoom { zoom_quickness } => {
                push_consts.extend_from_slice(&zoom_quickness.to_ne_bytes());
            }
            &Transition::SquaresWire {
                smoothness,
                squares,
                direction,
            } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&squares[0].to_ne_bytes());
                push_consts.extend_from_slice(&squares[1].to_ne_bytes());
                push_consts.extend_from_slice(&direction[0].to_ne_bytes());
                push_consts.extend_from_slice(&direction[1].to_ne_bytes());
            }
            &Transition::Squeeze { color_separation } => {
                push_consts.extend_from_slice(&color_separation.to_ne_bytes());
            }
            &Transition::StereoViewer {
                zoom,
                corner_radius,
            } => {
                push_consts.extend_from_slice(&zoom.to_ne_bytes());
                push_consts.extend_from_slice(&corner_radius.to_ne_bytes());
            }
            &Transition::Swap {
                reflection,
                perspective,
                depth,
            } => {
                push_consts.extend_from_slice(&reflection.to_ne_bytes());
                push_consts.extend_from_slice(&perspective.to_ne_bytes());
                push_consts.extend_from_slice(&depth.to_ne_bytes());
            }
            &Transition::TvStatic { offset } => {
                push_consts.extend_from_slice(&offset.to_ne_bytes());
            }
            &Transition::UndulatingBurnOut {
                smoothness,
                center,
                color,
            } => {
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
                push_consts.extend_from_slice(&center[0].to_ne_bytes());
                push_consts.extend_from_slice(&center[1].to_ne_bytes());
                push_consts.extend_from_slice(&color[0].to_ne_bytes());
                push_consts.extend_from_slice(&color[1].to_ne_bytes());
                push_consts.extend_from_slice(&color[2].to_ne_bytes());
            }
            &Transition::WaterDrop { amplitude, speed } => {
                push_consts.extend_from_slice(&amplitude.to_ne_bytes());
                push_consts.extend_from_slice(&speed.to_ne_bytes());
            }
            &Transition::Wind { size } => {
                push_consts.extend_from_slice(&size.to_ne_bytes());
            }
            &Transition::WindowSlice { count, smoothness } => {
                push_consts.extend_from_slice(&count.to_ne_bytes());
                push_consts.extend_from_slice(&smoothness.to_ne_bytes());
            }
            &Transition::ZoomLeftWipe { zoom_quickness } => {
                push_consts.extend_from_slice(&zoom_quickness.to_ne_bytes());
            }
            &Transition::ZoomRightWipe { zoom_quickness } => {
                push_consts.extend_from_slice(&zoom_quickness.to_ne_bytes());
            }
            Transition::BowTieHorizontal
            | Transition::BowTieVertical
            | Transition::CannabisLeaf
            | Transition::CoordFromIn
            | Transition::CrossWarp
            | Transition::Dreamy
            | Transition::Fade
            | Transition::GlitchDisplace
            | Transition::GlitchMemories
            | Transition::Heart
            | Transition::InvertedPageCurl
            | Transition::Luma { .. }
            | Transition::LeftRight
            | Transition::Multiply
            | Transition::RandomNoisex
            | Transition::Rotate
            | Transition::ScaleIn
            | Transition::Swirl
            | Transition::TangentMotionBlur
            | Transition::TopBottom
            | Transition::WindowBlinds
            | Transition::WipeDown
            | Transition::WipeLeft
            | Transition::WipeRight
            | Transition::WipeUp
            | Transition::ZoomInCircles => {}
        };

        // TODO: Handle displacement and luma in an if case, below
        render_graph
            .begin_pass(format!("transition {transition_ty:?}"))
            .bind_pipeline(&*pipeline)
            .read_descriptor(0, a_image)
            .read_descriptor(1, b_image)
            .write_descriptor(2, dest_image)
            .record_compute(move |compute| {
                compute.push_constants(push_consts.as_slice());
                compute.dispatch(dest_info.width, dest_info.height, 1);
            });

        dest_image
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TransitionType {
    Angular,
    Bounce,
    BowTieHorizontal,
    BowTieVertical,
    BowTieWithParameter,
    Burn,
    ButterflyWaveScrawler,
    CannabisLeaf,
    Circle,
    CircleCrop,
    CircleOpen,
    ColorDistance,
    ColorPhase,
    CoordFromIn,
    CrazyParametricFun,
    Crosshatch,
    CrossWarp,
    CrossZoom,
    Cube,
    Directional,
    DirectionalEasing,
    DirectionalWarp,
    DirectionalWipe,
    Displacement,
    DoomScreen,
    Doorway,
    Dreamy,
    DreamyZoom,
    FadeColor,
    Fade,
    FadeGrayscale,
    FilmBurn,
    Flyeye,
    GlitchDisplace,
    GlitchMemories,
    GridFlip,
    Heart,
    Hexagonalize,
    InvertedPageCurl,
    Kaleidoscope,
    LeftRight,
    LinearBlur,
    Luma,
    LuminanceMelt,
    Morph,
    Mosaic,
    Multiply,
    Overexposure,
    Perlin,
    Pinwheel,
    Pixelize,
    PolarFunction,
    PolkaDotsCurtain,
    PowerKaleido,
    Radial,
    RandomNoisex,
    RandomSquares,
    Ripple,
    Rotate,
    RotateScale,
    ScaleIn,
    SimpleZoom,
    SquaresWire,
    Squeeze,
    StereoViewer,
    Swap,
    Swirl,
    TangentMotionBlur,
    TopBottom,
    TvStatic,
    UndulatingBurnOut,
    WaterDrop,
    Wind,
    WindowBlinds,
    WindowSlice,
    WipeDown,
    WipeLeft,
    WipeRight,
    WipeUp,
    ZoomInCircles,
    ZoomLeftWipe,
    ZoomRightWipe,
}
