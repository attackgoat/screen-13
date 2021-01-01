# Screen 13

[![Crates.io](https://img.shields.io/crates/v/screen-13.svg)](https://crates.io/crates/screen-13)
[![Docs.rs](https://docs.rs/screen-13/badge.svg)](https://docs.rs/screen-13)
[![LoC](https://tokei.rs/b1/github/attackgoat/screen-13?category=code)](https://github.com/attackgoat/screen-13)

Screen 13 is an easy-to-use 2D/3D rendering engine in the spirit of QBasic.

## Overview

Programs made using Screen 13 are built as regular executables using an _optional_ design-time asset baking process. Screen 13 provides all asset-baking logic and aims to provide wide support for texture formats, vertex formats, and other associated data. Baked assets are stored in `.pak` files.

Screen 13 is based on the [`gfx-rs`](https://github.com/gfx-rs/gfx) project, and as such targets native Vulkan, Metal, DirectX 12, OpenGL, WebGL, Android, and iOS targets, among others.

### Goals

Screen 13 aims to provide a simple to use, although opinionated, ecosystem of tools and code that enable very high performance portable graphics programs for developers using the Rust programming language.

_Single Threaded:_ Although some things can be shared amongst other threads, such as disk and network IO, the main graphics API of Screen 13 does not support multiple threads. This is a conscious decision to limit complexity while optimizing for the 98% of programs that use a "main thread" methodology. I am open to changing this if the proposed API is easy to use and high performance. Perhaps it's as easy as a cargo manifest feature, not sure.

_Just Enough:_ Only core 2D and 3D rendering features are included, along with window event handling and window-based input. Additional things, such as an entity component system, physics, sound, and gamepad input must be handled by your code.

## Asset Baking

Asset baking is the process of converting files from their native file formats into a runtime-ready format that is optimized for both speed and size. Currently Screen 13 uses a single file (or single HTTP/S endpoint) for all runtime assets. Assets are baked from `.toml` files which you can find examples of in the `examples/content` directory.

## Quick Start

Included are some examples you might find helpful:

- `basic.rs` - Displays 'Hello, World!' on the screen. Please start here.
- `ecs.rs` - Example of integration with a third-party ECS library ([`hecs`](https://crates.io/crates/hecs), which is _excellent_).
- `headless.rs` - Rendering without an operating system window, saving to disk.
- `triangle.rs` - Loads a textured triangle at runtime, with no associated `.pak` file.

Some examples require an associated asset `.pak` file in order to run, so you will need to run the example like so:

```bash
cargo run examples/content/basic.toml
cargo run --example basic
```

These commands do the following:

- Build the Screen 13 engine (_runtime_) and executable code (_design-time_)
- Bake the assets from `basic.toml` into `basic.pak`
- Runs the `basic` example (Close window to exit)

See the example code for more information, including a helpful [getting started guide](examples/README.md).

## Roadmap/Status/Notes

This engine is very young and is likely to change as development continues.

- Requires [Rust](https://www.rust-lang.org/) 1.45 _or later_
- _Design-time_ Asset Baking:
  - Animation - **Rotations only**, no scaling, morph targets, or root motion
  - Bitmaps
    - Wide format support: .png, .jpg, _etc..._
    - 1-4 channel support
    - Unpacked using GPU compute hardware at runtime
  - Blobs (raw file byte vectors)
  - Language file (locale to key/value dictionary)
  - Material file
    - Supports metalness/roughness workflow with hardware optimized material data
  - Models - **.gltf** or **.glb** only
    - Requires `POSITION` and `TEXTURE0`
    - Static or skinned
    - Mesh name filtering/renaming/un-naming
  - Scenes
- _Runtime_ Asset `.pak` File:
  - Easy reading of assets
  - Configurable `compression`
    - `snap` is really good
    - `brotli` is amazing, but it has a bug at the moment and fails to read properly
- Rendering:
  - Deferred renderer - **in progress**
  - Forward renderer - **not started**
  - Roadmap:
    - Today: Each graphics operation starts and finishes recording a new gfx-hal command buffer, and then submits it
    - Today: All render pass/graphics pipeline/compute pipeline/layouts are hard-coded in the `gpu::def` (definition) module
    - Today: Each graphic operation uses the `def` instances and other types to operate the command buffer directly
    - Soon: Command buffers will be opened and closed dynamically, lifting ownership of command buffers (and queues) up a level
    - Later: Each graphic operation will record what it wants, but all graphic operations before a command buffer submit will be grouped
    - Later: Resources currently in `def` will be created at runtime based on the operation graph
    - Later: Render passes will be constructed dynamically as well
- General:
  - TODO: fonts, models, textures, etc... should be loadable at runtime from regular files

## Optional features

Screen 13 puts a lot of functionality behind optional features in order to optimize compile time for the most common use cases. The following features are available.

_NOTE_: The deferred and forward renderers have separate code paths and you can choose either on a render-by-render basis.

- **`debug-names`** — Name parameter added to most graphics calls, integrates with your graphics debugger.
- **`deferred-renderer`** *(enabled by default)* — Ability to draw models and lights using a deferred technique.
- **`forward-renderer`** *(enabled by default)* — Same as the deferred renderer, but using a forward technique.

## Content Baking Procedures

A main project `.toml` file is required. All content loaded at runtime must be present in this file.

Additional `.toml` asset files are referenced using either relative (`../path/file.ext`) or absolute (`/path/file.ext`) format, where the root is the same directory as the main project `.toml` file.

### Brotli Compression

Higher compression ratio but somewhat slow during compression. Compresses 108mb to 3.8mb in a real-world test.

```toml
[content]
compression = 'brotli'
buf_size = 4096
quality = 10
window_size = 20
```

### Snap Compression

Faster during compression and lower compression ratio compared to Brotli. Compresses 108mb to 12mb in a real-world test. Best for use when re-building assets often.

```toml
[content]
compression = 'snap'
```

## History

As a child I was given access to a computer that had GW-Basic; and later one with QBasic. All of my favorite programs started with:

```basic
CLS
SCREEN 13
```

These commands cleared the screen of text and setup a 320x200 256-color paletized color video mode. There were other video modes available, but none of them had the 'magic' of 256 colors.

Additional commands QBasic offered, such as `DRAW`, allowed you to build very simple games incredibly quickly because you didn't have to grok the enirety of linking and compiling in order get things done. I think we should have options like this today, and this project aims to allow future developers to have the same ability to get things done quickly while using modern tools.
