# Screen 13

[![Crates.io](https://img.shields.io/crates/v/screen-13.svg)](https://crates.io/crates/screen-13)
[![Docs.rs](https://docs.rs/screen-13/badge.svg)](https://docs.rs/screen-13)
[![LoC](https://tokei.rs/b1/github/attackgoat/screen-13?category=code)](https://github.com/attackgoat/screen-13)

_Screen 13_ is an easy-to-use Vulkan rendering engine in the spirit of
_[QBasic](https://en.wikipedia.org/wiki/QBasic)_.

```toml
[dependencies]
screen-13 = "0.11"
```

## Overview

_Screen 13_ provides a high performance [Vulkan](https://www.vulkan.org/) driver using smart
pointers. The driver may be created manually for headless rendering or automatically using the
built-in window abstraction:

```rust
use screen_13_window::{Window, WindowError};

fn main() -> Result<(), WindowError> {
    Window::new()?.run(|frame| {
        // It's time to do some graphics! 😲
    })
}
```

## Usage

_Screen 13_ provides a fully-generic render graph structure for simple and statically
typed access to all the resources used while rendering. The `RenderGraph` structure allows Vulkan
smart pointer resources to be bound as "nodes" which may be used anywhere in a graph. The graph
itself is not tied to swapchain access and may be used to execute general command streams.

Features of the render graph:

 - Compute, graphic, and ray-trace pipelines
 - Automatic Vulkan management (render passes, subpasses, descriptors, pools, _etc._)
 - Automatic render pass scheduling, re-ordering, merging, with resource aliasing
 - Interoperable with existing Vulkan code
 - Optional [shader hot-reload](contrib/screen-13-hot/README.md) from disk

```rust
render_graph
    .begin_pass("Fancy new algorithm for shading a moving character who is actively on fire")
    .bind_pipeline(&gfx_pipeline)
    .read_descriptor(0, some_image)
    .read_descriptor(1, another_image)
    .read_descriptor(3, some_buf)
    .clear_color(0, swapchain_image)
    .store_color(0, swapchain_image)
    .record_subpass(move |subpass| {
        subpass.push_constants(some_u8_slice);
        subpass.draw(6, 1, 0, 0);
    });
```
### Debug Logging

This crate uses [`log`](https://crates.io/crates/log) for low-overhead logging.

To enable logging, set the `RUST_LOG` environment variable to `trace`, `debug`, `info`, `warn` or
`error` and initialize the logging provider of your choice. Examples use
[`pretty_env_logger`](https://docs.rs/pretty_env_logger/latest/pretty_env_logger/).

_You may also filter messages, for example:_

```bash
RUST_LOG=screen_13::driver=trace,screen_13=warn cargo run --example ray_trace
```

```
TRACE screen_13::driver::instance > created a Vulkan instance
DEBUG screen_13::driver::physical_device > physical device: NVIDIA GeForce RTX 3090
DEBUG screen_13::driver::physical_device > extension "VK_KHR_16bit_storage" v1
DEBUG screen_13::driver::physical_device > extension "VK_KHR_8bit_storage" v1
DEBUG screen_13::driver::physical_device > extension "VK_KHR_acceleration_structure" v13
...
```

### Performance Profiling

This crates uses [`profiling`](https://crates.io/crates/profiling) and supports multiple profiling
providers. When not in use profiling has zero cost.

To enable profiling, compile with one of the `profile-with-*` features enabled and initialize the
profiling provider of your choice.

_Example code uses [puffin](https://crates.io/crates/puffin):_

```bash
cargo run --features profile-with-puffin --release --example vsm_omni
```

<img src=".github/img/profile.png" alt="Flamegraph of performance data" width=30%>

## Quick Start

Included are some examples you might find helpful:

- [`hello_world.rs`](contrib/screen-13-window/examples/hello_world.rs) — Displays a window on the screen. Please start here.
- [`triangle.rs`](examples/triangle.rs) — Shaders and full setup of index/vertex buffers; < 100 LOC.
- [`shader-toy/`](examples/shader-toy) — Recreation of a two-pass shader toy using the original
  shader code.

See the [example code](examples/README.md), 
[documentation](https://docs.rs/screen-13/latest/screen_13/), or helpful
[getting started guide](examples/getting-started.md) for more information.

**_NOTE:_** Required development packages and libraries are listed in the _getting started guide_.
All new users should read and understand the guide.

## History

As a child I was given access to a computer that had _GW-Basic_; and later one with _QBasic_. All of
my favorite programs started with:

```basic
CLS
SCREEN 13
```

These commands cleared the screen of text and setup a 320x200 256-color paletized video mode. There
were other video modes available, but none of them had the 'magic' of 256 colors.

Additional commands _QBasic_ offered, such as `DRAW`, allowed you to build simple games quickly
because you didn't have to grok the entirety of compiling and linking. I think we should have
options like this today, and so I started this project to allow future developers to have the
ability to get things done quickly while using modern tools.

### Inspirations

_Screen 13_ was built from the learnings and lessons shared by others throughout our community. In
particular, here are some of the repositories I found useful:

 - [Bevy](https://bevyengine.org/): A refreshingly simple data-driven game engine built in Rust
 - [Granite](https://github.com/Themaister/Granite) - Open-source Vulkan renderer
 - [Kajiya](https://github.com/EmbarkStudios/kajiya) - Experimental real-time global illumination
   renderer made with Rust and Vulkan
