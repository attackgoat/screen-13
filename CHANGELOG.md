# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.2] - 2022-06-01

### Added

- Additional memory mapping functions to `Buffer` structure

### Changed

- `BlendMode` graphic pipeline enumeration is now a structure with full options

## [0.3.1] - 2022-05-27

### Added

- Bindless descriptor support (unsized arrays in shader code) and example

### Fixed

- Improve swapchain image flag handling

## [0.3.0] - 2022-05-20

### Added

- Ray tracing support
- Subpass API, additional examples

### Removed

- Pak file functionality moved to `pak` [crate](https://crates.io/crates/pak)

## 0.2.1 - _Unreleased_

### Added

- Dear ImGui library and example
- Bitmapped text rendering

### Changed

- Pak file baking is now multi-threaded; assets still only get packed exactly once

### Removed

- `CommandChain` structure functionality is now found on the `RenderGraph` structure

## [0.2.0] - 2022-02-08

### Added

- Render Graph module, bindings, nodes, and executions: with render pass merging/re-ordering/etc
- `CommandChain` structure

### Changed

- Driver now directly based on vulkan, having removed support for the deprecated Gfx-Hal library
- Lease/pool functionality simplified: leases are now obtained through a common interface using info
- `Engine`/`Program` structures have been merged into a simpler EventLoop structure

### Removed

- _Screen 13_ file-based configuration: use DriverConfig now
- `Gpu` and `Render` structures: use `RenderGraph` and `ImageNode` now
- Existing bitmap/draw/text/write/etc operations: functionality replaced and in some cases TODO

## 0.1.9 - _Unreleased_

### Added

- Implementation of `draw` functionality
- Implementation of `text` functionality

## 0.1.8 - _Unreleased_

### Added

- Features: `auto-cull`, `debug-names`, `deferred-3d`, and `forward-3d`
- Selectable `Arc` or `Rc` shared types

### Changed

- Allow `write` function to specify multiple writes per call
- Use builder pattern for all rendering commands
- Switched asset schema from JSON to TOML

## [0.1.0] - 2020-07-05

### Added

- Easy-to-use API designed to allow developers to create graphics programs which run on many
  platforms and require no bare-metal graphics API knowledge
- "Hello, world!" example using a bitmapped font

[Unreleased]: https://github.com/attackgoat/screen-13/compare/0200335...HEAD
[0.1.0]: https://crates.io/crates/screen-13/0.1.0
[0.2.0]: https://crates.io/crates/screen-13/0.2.0
[0.3.0]: https://crates.io/crates/screen-13/0.3.0
[0.3.1]: https://crates.io/crates/screen-13/0.3.1
[0.3.2]: https://crates.io/crates/screen-13/0.3.2