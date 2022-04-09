# Screen 13

[![Crates.io](https://img.shields.io/crates/v/screen-13.svg)](https://crates.io/crates/screen-13)
[![Docs.rs](https://docs.rs/screen-13/badge.svg)](https://docs.rs/screen-13)
[![LoC](https://tokei.rs/b1/github/attackgoat/screen-13?category=code)](https://github.com/attackgoat/screen-13)

_Screen 13_ is an easy-to-use 2D/3D rendering engine in the spirit of
_[QBasic](https://en.wikipedia.org/wiki/QBasic)_.

## Overview

_Screen 13_ provides a thin [Vulkan 1.1](https://www.vulkan.org/) driver using smart pointers.

Features of the Vulkan driver:

 - Lifetime management calls `free` for you
 - Resource information comes with each smart pointer
 - Easy-to-use hashable/orderable types (no raw pointers)

Example usage:

```rust
let window = ...your winit window...
let cfg = Default::default();
let (width, height) = (320, 200);
let driver = Driver::new(&window, cfg, width, height)?;

unsafe {
    // Let's do low-level stuff using the provided ash::Device
    driver.device.create_fence(...);
}
```

### Render Graph

_Screen 13_ provides a fully-generic render graph structure for simple and statically
typed access to all the resources used while rendering. The `RenderGraph` structure allows Vulkan
smart pointer resources to be bound as "nodes" which may be used anywhere in a graph. The graph
itself is not tied to swapchain access and may be used from a headless environment too.

Features of the render graph:

 - Compute, Graphic, and Ray-trace pipelines
 - You specify _code_ which runs on _input_ and creates _output_
 - Automatic Vulkan management (Render passes, subpasses, descriptors, pools, etc.)
 - Automatic render pass scheduling, re-ordering, merging, with resource aliasing

Example usage (_See [source](examples/shader-toy/src/main.rs) for variable values_):

```rust
render_graph
    .record_pass("Buffer A")
    .bind_pipeline(&buf_pipeline)
    .read_descriptor(0, input)
    .read_descriptor(1, noise_image)
    .read_descriptor(2, flowers_image)
    .read_descriptor(3, blank_image)
    .clear_color(0)
    .store_color(0, output)
    .push_constants(push_consts)
    .draw(move |device, cmd_buf, _| unsafe {
        device.cmd_draw(cmd_buf, 6, 1, 0, 0);
    });
```

### Event Loop

_Screen 13_ provides an event loop abstraction which helps you setup and display images easily. Also
included are keyboard, mouse, and typing input helpers.

Example usage:

```rust
fn main() -> Result<(), DisplayError> {
    let event_loop = EventLoop::new().build()?;

    event_loop.run(|frame| {
        // Draw using frame.render_graph here!
    })
}
```

### Pak File Format

Programs made using _Screen 13_ are built as regular executables using an _optional_ design-time
asset baking process. _Screen 13_ provides all asset-baking logic and aims to provide wide support
for texture formats, vertex formats, and other associated data. Baked assets are stored in `.pak`
files.

Features of the `.pak` file format:

- Individually compressed assets
- Baking process is multi-threaded and heavily cached
- Supports `.gltf`/`.glb` with LOD, meshlets, cache/fetch optimizations, and more
- Material system (including baking of PBR data)

## Goals

_Screen 13_ aims to provide a simple to use, although opinionated, ecosystem of tools and code that
enable very high performance portable graphics programs for developers using the Rust programming
language.

_Just Enough:_ Only core 2D and 3D rendering features are included, along with window event handling
and window-based input. Additional things, such as an entity component system, physics, sound, and
gamepad input must be handled by your code.

## Quick Start

Included are some examples you might find helpful:

- [`hello_world.rs`](examples/hello_world.rs) — Displays a window on the screen. Please start here.
- [`bake_pak.rs`](examples/bake_pak.rs) — Bakes a simple `.pak` file from a `.toml` definition.
- [`shader-toy/`](examples/shader-toy) — Recreation of a two-pass shader toy using the original
  shader code.

See the example code for more information, including a helpful
[getting started guide](examples/README.md).

**_NOTE:_** Required development packages and libraries are listed in the _getting started guide_.
All new users should read and understand the guide.

## Optional Features

_Screen 13_ puts a lot of functionality behind optional features in order to optimize compile time
for the most common use cases. The following features are available.

- **`pak`** *(enabled by default)* — Ability read `.pak` files.
- **`bake`** — Ability to write `.pak` files, enables `pak` feature.

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

### Insipirations

_`Screen 13`_ was built from the learnings and lessons shared by others throughout our community. In
particular, here are some of the repositories I found useful:

 - [Bevy](https://bevyengine.org/): A refreshingly simple data-driven game engine built in Rust
 - [Granite](https://github.com/Themaister/Granite) - Open-source Vulkan renderer
 - [Kajiya](https://github.com/EmbarkStudios/kajiya) - Experimental real-time global illumination
   renderer made with Rust and Vulkan
