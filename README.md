# Screen 13

[![Crates.io](https://img.shields.io/crates/v/screen-13.svg)](https://crates.io/crates/screen-13)
[![Docs.rs](https://docs.rs/screen-13/badge.svg)](https://docs.rs/screen-13)

Screen 13 is an easy-to-use 3D game engine in the spirit of QBasic.

## Overview

Games made using Screen 13 are built as regular executables using a design-time asset baking process. Screen 13 provides all asset-baking logic and aims to, but currently does not, provide wide support for texture formats, vertex formats, and other associated data. Baked assets are stored in `.pak` files.

## Asset Baking

Asset baking is the industry-standard process of converting files from their native file formats into a runtime-ready format that is optimized for both speed and size. Currently Screen 13 uses a single file (or single HTTP/S endpoint) for all runtime assets. Assets are baked from `.txt` and `.json` files which you can find examples of in the `examples/content` directory.

## Quick Start

Included are four examples you might find helpful, in order of complexity:

- `basic.rs` - Displays 'Hello, World!' on the screen. Please start here.
- `nibbles.rs` - A game built using lines as the only graphical element. (Done but engine doesn't draw yet)
- `gorilla.rs` - A game demonstrating 2D bitmaps/tilemaps. (Not started yet)
- `wasm.rs` - A 3D technology demonstration; runs in your web browser. (Not started yet)

Each of these examples requires an associated asset `.pak` file in order to run, so you will need to run the examples like so:

```bash
cargo run examples/content/basic.txt
cargo run --example basic
```

These commands do the following:

- Build the Screen 13 engine (_runtime_) and executable code (_design-time_)
- Bake the assets from `basic.txt` into `basic.pak`
- Runs the `basic` example (Press ESC to exit)

## Roadmap/Status/Notes

This engine is very young and is likely to change as development continues.

- Asset .pak file baking: Needs work, currently written in a script-like or procedural style and should be refactored to become much more general purpose
- Asset .pak file runtime: 75% complete. Needs byte stream compression and implemetation of HTTP/S support.
- Debug names should maybe be a Cargo.toml "feature" for games that aren't attempting to support debuggability via graphics API capturing tools such as RenderDoc. The way it is right now lots of API calls require a string you must attribute with the debug-assertions if-config attribute.
- There are countless TODO's scattered in this codebase; this project started as a closed-source personal project and so Github issues and such for tracking things were not the original method I used. Feel free to replace TODO's by opening a matching Issue or just removing outdated TODO information.
- Drawing lines, bitmaps, 3D models, lights (and shadows): I recently ripped out all this code in order to add a compilation stage after you submit rendering commands. This allows for proper z-order painting and batching to reduce GPU resource-switching. It is not complete yet and requires more work.
- Input: Keyboard has been started but the design is not very good. Mouse input is to-do. Game controllers and joysticks are planned.

## History

As a child I was given access to a computer that had GW-Basic; and later one with QBasic. All of my favorite programs started with:

```
CLS
SCREEN 13
```

These commands cleared the screen of text and setup a 320x200 256-color paletized color video mode. There were other video modes available, but none of them had the 'magic' of 256 colors.

Additional commands QBasic offered, such as `DRAW`, allowed you to build very simple games incredibly quickly because you didn't have to grok the enirety of linking and compiling in order get things done. I think we should have options like this today, and this project aims to allow future developers to have the same ability to get things done quickly while using modern tools.

## Special Shout-out to Kenney Vleugels

The example code uses a few of the awesome 2D, 3D, and audio assets provided by [Kenney Vleugels](https://www.kenney.nl/). These assets have been generously provided under the CC0 1.0 license and we are therefore able to share them with you in this repository. Please consider supporting Kenney for the excellent work and for what it is doing to help the game development community.

_NOTE:_ If you look within the `examples/content/kenney.nl` directory you will find the packages we are using, however these packages are not the complete packages offered on their website because we have removed any assets that are not used in the examples. If you want to use these assets in your game we recommend you get your copy directly from their website.

## Notes

- Run your game with the `RUST_LOG` environment variable set to `trace` for detailed debugging messages
- Make all panics/todos/unreachables and others only have messages in debug builds?
- Consider removing the extra derived things
- Create new BMFont files on Windows using [this](http://www.angelcode.com/products/bmfont/)
- Regenerate files by cd'ing to correct directory and run this:
  - "c:\Program Files (x86)\AngelCode\BMFont\bmfont.com" -c SmallFonts-12px.bmfc -o SmallFonts-12px.fnt
  - "c:\Program Files (x86)\AngelCode\BMFont\bmfont.com" -c SmallFonts-10px.bmfc -o SmallFonts-10px.fnt
