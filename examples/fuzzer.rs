/*

Kind of an example, kind of a test - not good looking
Used for code coverage with https://github.com/mozilla/grcov

First time:
    rustup component add llvm-tools-preview

In a separate terminal:
    export RUSTFLAGS="-Cinstrument-coverage"
    cargo build --example fuzzer

Next:
    export LLVM_PROFILE_FILE="fuzzer-%p-%m.profraw"
    target/debug/examples/fuzzer

*/
use {inline_spirv::inline_spirv, rand::random, screen_13::prelude_arc::*};

fn main() -> Result<(), DisplayError> {
    pretty_env_logger::init();

    let screen_13 = EventLoop::new().debug(true).build()?;
    let _cache = HashPool::new(&screen_13.device);

    let mut frames_remaining = 100;

    screen_13.run(|mut frame| {
        // We stop fuzzing after 100 frames
        frames_remaining -= 1;
        if frames_remaining == 0 {
            *frame.will_exit = true;
        }

        // We fuzz a random amount of randomly selected operations per frame
        let operations_per_frame: u8 = random();
        let operation: u8 = random();
        for _ in 0..operations_per_frame {
            match operation % 2 {
                op if op == 0 => record_compute_no_op(&mut frame),
                op if op == 1 => record_graphic_wont_merge(&mut frame),
                _ => unreachable!(),
            }
        }

        // We are not testing the swapchain - so always clear it
        frame.render_graph.clear_color_image(frame.swapchain_image);
    })?;

    debug!("OK");

    Ok(())
}

fn record_compute_no_op(frame: &mut screen_13::FrameContext<ArcK>) {
    let pipeline = compute_pipeline(
        frame.device,
        inline_spirv!(
            r#"
            #version 460 core

            void main() {
            }
            "#,
            comp
        )
        .as_slice(),
    );
    frame
        .render_graph
        .begin_pass("no-op")
        .bind_pipeline(&pipeline)
        .record_compute(|compute| {
            compute.dispatch(0, 0, 0);
        });
}

fn record_graphic_wont_merge(frame: &mut FrameContext) {
    let pipeline = graphic_vert_frag_pipeline(
        frame.device,
        GraphicPipelineInfo::default(),
        inline_spirv!(
            r#"
            #version 460 core

            void main() {
            }
            "#,
            vert
        )
        .as_slice(),
        inline_spirv!(
            r#"
            #version 460 core

            layout(location = 0) out vec4 color;

            void main() {
            }
            "#,
            frag
        )
        .as_slice(),
    );

    // These two passes have common writes but are otherwise regular - they won't get merged
    frame
        .render_graph
        .begin_pass("a")
        .bind_pipeline(&pipeline)
        .store_color(0, frame.swapchain_image)
        .record_subpass(|subpass| {
            subpass.draw(0, 0, 0, 0);
        });
    frame
        .render_graph
        .begin_pass("b")
        .bind_pipeline(&pipeline)
        .store_color(0, frame.swapchain_image)
        .record_subpass(|subpass| {
            subpass.draw(0, 0, 0, 0);
        });
}

// Below are convenience functions used to create test data

fn compute_pipeline(device: &Shared<Device>, source: &'static [u32]) -> Shared<ComputePipeline> {
    use std::{cell::RefCell, collections::HashMap};

    thread_local! {
        static TLS: RefCell<HashMap<&'static [u32], Shared<ComputePipeline>>> = Default::default();
    }

    TLS.with(|tls| {
        Shared::clone(tls.borrow_mut().entry(source).or_insert_with(|| {
            Shared::new(ComputePipeline::create(device, ComputePipelineInfo::new(source)).unwrap())
        }))
    })
}

fn graphic_vert_frag_pipeline(
    device: &Shared<Device>,
    info: impl Into<GraphicPipelineInfo>,
    vert_source: &'static [u32],
    frag_source: &'static [u32],
) -> Shared<GraphicPipeline> {
    use std::{cell::RefCell, collections::HashMap};

    #[derive(Eq, Hash, PartialEq)]
    struct Key {
        info: GraphicPipelineInfo,
        vert_source: &'static [u32],
        frag_source: &'static [u32],
    }

    thread_local! {
        static TLS: RefCell<HashMap<Key, Shared<GraphicPipeline>>> = Default::default();
    }

    let info = info.into();

    TLS.with(|tls| {
        Shared::clone(
            tls.borrow_mut()
                .entry(Key {
                    info: info.clone(),
                    vert_source,
                    frag_source,
                })
                .or_insert_with(move || {
                    Shared::new(
                        GraphicPipeline::create(
                            device,
                            info,
                            [
                                Shader::new_vertex(vert_source),
                                Shader::new_fragment(frag_source),
                            ],
                        )
                        .unwrap(),
                    )
                }),
        )
    })
}
