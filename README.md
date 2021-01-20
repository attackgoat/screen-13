# Screen 13

[![Crates.io](https://img.shields.io/crates/v/screen-13.svg)](https://crates.io/crates/screen-13)
[![Docs.rs](https://docs.rs/screen-13/badge.svg)](https://docs.rs/screen-13)
[![LoC](https://tokei.rs/b1/github/attackgoat/screen-13?category=code)](https://github.com/attackgoat/screen-13)

_Screen 13_ is an easy-to-use 2D/3D rendering engine in the spirit of _QBasic_.

## Overview

Programs made using _Screen 13_ are built as regular executables using an _optional_ design-time
asset baking process. _Screen 13_ provides all asset-baking logic and aims to provide wide support
for texture formats, vertex formats, and other associated data. Baked assets are stored in `.pak`
files.

_Screen 13_ is based on the [_`gfx-rs`_](https://github.com/gfx-rs/gfx) project, and as such targets
native Vulkan, Metal, DirectX 12, OpenGL, WebGL, Android, and iOS targets, among others.

### Goals

_Screen 13_ aims to provide a simple to use, although opinionated, ecosystem of tools and code that
enable very high performance portable graphics programs for developers using the Rust programming
language.

_Just Enough:_ Only core 2D and 3D rendering features are included, along with window event handling
and window-based input. Additional things, such as an entity component system, physics, sound, and
gamepad input must be handled by your code.

## Quick Start

Included are some examples you might find helpful:

- [`basic.rs`](examples/basic.rs) — Displays 'Hello, World!' on the screen. Please start here.
- [`ecs.rs`](examples/ecs.rs) — Example of integration with a third-party ECS library
  ([_`hecs`_](https://crates.io/crates/hecs), which is _excellent_).
- [`headless.rs`](examples/headless.rs) — Renders without an operating system window, saves to disk.
- [`triangle.rs`](examples/triangle.rs) — Loads a textured triangle at runtime, with no associated
  `.pak` file.
- [`wasm/`](examples/wasm) 🚧 — Runs in a web browser.

Some examples require an associated asset `.pak` file in order to run, so you will need to run the
example like so:

```bash
cargo run --release examples/res/basic.toml
cargo run --release --example basic
```

These commands do the following:

- Build the _Screen 13_ engine (_runtime_) and executable code (_design-time_)
- Bake the assets from `basic.toml` into `basic.pak`
- Runs the `basic` example (Close window to exit)

See the example code for more information, including a helpful
[getting started guide](examples/README.md).

**_NOTE:_** Required development packages and libraries are listed in the _getting started guide_.
All new users should read and understand the guide.

## Roadmap/Status/Notes

This engine is very young and is likely to change as development continues. Some features may be
unimplemented.

I expect this engine to remain unstable/poorly documented until version 0.2. Based on current
progress and my free time (this is just a hobby) I expect version 0.2 (stable) to be released
before June 2021. Feel free to chip in to speed that up!

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
    - Today: Each graphics operation starts and finishes recording a new gfx-hal command buffer, and
      then submits it
    - Today: All render pass/graphics pipeline/compute pipeline/layouts are hard-coded in the
      `gpu::def` (definition) module
    - Today: Each graphic operation uses the `def` instances and other types to operate the command
      buffer directly
    - Soon: Command buffers will be opened and closed dynamically, lifting ownership of command
      buffers (and queues) up a level
    - Later: Each graphic operation will record what it wants, but all graphic operations before a
      command buffer submit will be grouped
    - Later: Resources currently in `def` will be created at runtime based on the operation graph
    - Later: Render passes will be constructed dynamically as well
    - Later: Gently copy the light binning magic from Granite?
- General:
  - TODO: fonts, models, textures, etc... should be loadable at runtime from regular files

## Optional features

_Screen 13_ puts a lot of functionality behind optional features in order to optimize compile time
for the most common use cases. The following features are available.

**_NOTE:_** The deferred and forward renderers have separate code paths and you can choose either on
a render-by-render basis.

- **`auto-cull`** — Enables automatic draw call camera frustum culling.
- **`debug-names`** — Name parameter added to most graphics calls, integrates with your graphics
  debugger.
- **`blend-modes`** *(enabled by default)* —
  [Normal](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.BlendMode.html#variant.Normal),
  [Add](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.BlendMode.html#variant.Add),
  [Subtract](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.BlendMode.html#variant.Subtract),
  [Color Burn](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.BlendMode.html#variant.ColorBurn),
  _etc..._
- **`deferred-3d`** *(enabled by default)* — Ability to draw models and lights using a deferred
  technique.
- **`forward-3d`** 🚧 *(enabled by default)* — Same as the deferred renderer, but using a forward
  technique.
- **`low-power`** — Prefer using integrated graphics hardware, instead of higher-power adapters.
- **`mask-modes`** 🚧 *(enabled by default)* — Ability to use
  [image masking](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.MaskMode.html) functions.
- **`matte-modes`** 🚧 *(enabled by default)* — Ability to use
  [image matting](https://docs.rs/screen-13/0.1.8/screen_13/gpu/enum.MatteMode.html) functions.
- **`multi-monitor`** 🚧 — Extends the `Screen` trait to support multiple viewports.
- **`xr`** 🚧 — Additional types and functions related to augmented and virtual reality.

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
