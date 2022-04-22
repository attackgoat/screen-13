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
use screen_13::prelude_arc::*;

fn main() -> Result<(), DisplayError> {
    let screen_13 = EventLoop::new().debug(true).build()?;
    let _cache = HashPool::new(&screen_13.device);

    screen_13.run(|frame| {
        *frame.will_exit = true;

        frame
            .render_graph
            .begin_pass("a")
            .bind_pipeline(&compute_pipeline(
                frame.device,
                r#"
                #version 460 core

                layout(location = 0) out vec2 vk_TexCoord;

                void main() {
                    gl_Position = vec4(0, 0, 0, 1);
                    vk_TexCoord = vec2(0, 1);
                }
                "#,
            ))
            .record_compute(|c| {
                c.dispatch(0, 0, 0);
            });
    })
}

fn compute_pipeline(device: &Shared<Device>, source: &'static str) -> Shared<ComputePipeline> {
    use std::{cell::RefCell, collections::HashMap};

    thread_local! {
        static TLS: RefCell<HashMap<&'static str, Shared<ComputePipeline>>> = Default::default();
    }

    TLS.with(|tls| {
        Shared::clone(tls.borrow_mut().entry(source).or_insert_with(|| {
            Shared::new(
                ComputePipeline::create(device, ComputePipelineInfo::new([0u8].as_slice()))
                    .unwrap(),
            )
        }))
    })
}
