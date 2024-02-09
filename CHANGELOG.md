# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Expanded the number of functions and scopes profiled by the `profiling` crate
- Increase `PoolInfo::DEFAULT_RESOURCE_CAPACITY` from 4 to 16 in order to prevent excess resource
  creation

## [0.9.4] - 2024-02-07

### Changed

- Improved performance during render graph resolution: `vsm_omni` example now records frames 10%
  faster (~100 μs) and complex render graphs may be signifcantly more performant

## [0.9.3] - 2024-01-30

### Added

- `FifoPool` resource pool implementation
- Memory management functions and configurable bucket sizes for `Pool` implementations

### Fixed

- Compilation bug for `rustc` v1.75.0 on Mac OS

## [0.9.2] - 2024-01-23

### Changed

- Deprecated `EventLoop` surface format functions
- Updated `derive_builder` to v0.13
- Updated `gpu-allocator` to v0.25
- Updated `winit` to v0.29
- Updated `egui` to v0.25
- Updated `imgui-rs` to latest
  [`main`](https://github.com/imgui-rs/imgui-rs/tree/ca05418cb449dadaabf014487c5c965908dfcbdd)

## [0.9.1] - 2023-12-29

### Added

- Ability to select from available swapchain surface formats when creating an `EventLoop`
- Driver `surface` and `swapchain` modules (_and their types_) are now public API

### Changed

- Changed `KeyBuf` implementation functions to take values instead of borrows
- Updated `gpu-allocator` to v0.24
- Updated `spirq` to v1.0.2

## [0.9.0] - 2023-09-07

### Fixed

- Incorrect handling of images with multiple array layers during render passes
- Validation error related to `VK_KHR_surface` when using headless devices
- Shader modules of graphic pipelines cached by a render pass were not considered during lookup

### Added

- Support for performance profiling crates
- Queue family index is now a part of the API and allows for submission of render graph work using 
  secondary queue families
- Expose all Vulkan 1.0 properties via `PhysicalDevice::features_v1_0`
- `Device::format_properties` and `Device::image_format_properties` so user code may avoid
  calling unsafe `ash` functions
- `RenderGraph::node_device_address` function
- `contrib/screen-13-hot`: Shader compilation macro definition support
- Virtual reality example using OpenXR - see [`examples/vr`](examples/vr/README.md)
- Support for `VK_EXT_index_type_uint8`; use `device.physical_device.index_type_uint8_features.index_type_uint8` to check for support
- Manually configurable image samplers - see [`examples/image_sampler.rs`](examples/image_sampler.rs)

### Changed

- Device creation (and `EventLoop::build()`) no longer take a ray-tracing parameter; instead the
  device will be created and you should use
  `device.physical_device.ray_trace_features.ray_tracing_pipeline` to check for support
- Logical device (`Device`) structure has been moved to `screen_13::driver::device`
- Physical device feature and property structures have been moved to
  `screen_13::driver::physical_device`
- Re-ordered parameters of `RenderGraph` functions: `blit_image_region`, `blit_image_regions`, and
  `update_buffer_offset`
- Updated parameters of `RenderGraph` functions to be more efficient (`Into<Box<[_]>>` is now
  `AsRef<[_]>` and take values of `Copy`-types instead of borrows)
- `ResolverPool` trait has been moved from the `screen_13::graph` module to `screen_13`

### Removed

- `Driver` structure; use `Device::create_headless` directly
- `PhysicalDeviceDescriptorIndexingFeatures` and `FeatureFlags` as they are no longer required

## [0.8.1] - 2023-02-18

### Fixed

- Pipelines which use multiple descriptor sets (different `set =` values) sometimes trigger
  validation errors
- `contrib/screen-13-hot`: build error on Windows platform

### Added

- Custom vertex layout support - see [`examples/vertex_layout.rs`](examples/vertex_layout.rs)
- Enabled full set of Vulkan 1.1 and Vulkan 1.2 core features during device creation
- Ray query support with `ray_omni.rs` example
- Exposed existing command buffer implementation so that programs may wait for render graph GPU
  submissions to finish executing before reading the results with the CPU - see
  [`examples/cpu_readback.rs`](examples/cpu_readback.rs)
- `KeyBuf::is_down` helper function

### Changed

- `contrib/screen-13-egui`: Updated `egui` to v0.20

## [0.8.0] - 2022-12-28

### Added

- Shader hot-reload feature for compute, graphic and ray-trace pipelines (see examples)
- `Buffer` objects may be created with an alignment specified in `BufferInfo` (useful for shader
  binding tables)

### Changed

- `ComputePipeline::create` now takes three arguments: the device, info, and shader
- `ComputePipelineInfo` no longer contains shader information; use `Shader::new_compute` for that
  instead

## [0.7.1] - 2022-12-17

### Fixed

- Soundness issue in `AccelerationStructure::instance_slice` helper function

### Added

- Skeletal mesh animation demonstration in [`examples/skeletal-anim`](examples/skeletal-anim/README.md)

## [0.7.0] - 2022-12-05

### Fixed

- Validation error caused by image blit operations
- `multipass.rs` and other examples use unsupported image formats without checking for fallbacks

### Added

- `EventLoop` may be constructed with multiple hardware queues, see `desired_queue_count` and the
  new `multithread.rs` example

### Changed

- `Resolver::submit()` now takes a queue index instead of an instance; `Device::queue_count`
  provides the total number of queues available

## [0.6.5] - 2022-11-11

### Fixed

- Incorrectly skipped pipeline barriers on resources used in secondary render passes
- Semaphore in-use validation error when dropping swapchain
- Validation error caused by back-to-back image reads in auto-merged fragment shader passes
- Validation error caused by node access for the ALL_COMMANDS stage before graphic passes
- Multiple validation errors in the `example/` and `contrib/` code

### Added

- `bindless.rs` example using an unbounded image sampler array and draw indirect call

### Changed

- Leased resources now reference their pool using `Weak` reference counting to improve drop ordering

## [0.6.4] - 2022-10-31

### Fixed

- Framebuffer resolve functionality was implemented incorrectly, did not work
- Synchronization error when using compute written-resources in fragment shaders
- Validation error in `multipass.rs` example
- Unnecessary depth buffer store operations in `vsm_omni.rs` example

### Added

- Mutlisampled anti-aliasing example (MSAA)
- `attach_color` and `attach_depth_stencil` functions on `PipelinePassRef` when bound to a `GraphicPipeline` for attachments which would otherwise use `VK_ATTACHMENT_LOAD_OP_DONT_CARE`
- `node_info` function on `PassRef` and `PipelinePassRef` which may be accessed while recording passes

## [0.6.3] - 2022-10-25

### Fixed

- Panic when setting exclusive fullscreen if the monitor is set to less than maximum resolution
- Panic when overlapping push constant ranges in graphic and ray trace pipelines

### Added

- `bind_node` function on `PassRef` and `PipelinePassRef` which may be accessed while recording passes

### Changed

- Improved fullscreen experience: no extra decoration or briefly displayed small window
- Cursor re-displayed, if hidden, when event loop window loses focus

## [0.6.2] - 2022-10-20

### Fixed

- Crash/device lost while resizing the window
- Inconsistent frame timing on certain drivers
- Incorrect window size on certain drivers

### Added

- Fullscreen demostration in `vsm_omni` example using F11 and F12 keys
- Configurable frames-in-flight setting

## [0.6.1] - 2022-10-16

### Fixed

- Depth/stencil images are now cleared properly
- Multi-layer framebuffers work as intended
- Render graph resolver orders renderpasses correctly

### Added

- `default_view_info()` helper on `ImageInfo` to assist in defining new views
- Variance shadow mapping example using a filtered cubemap

## [0.6.0] - 2022-10-06

### Changed

- `clear_color` and `clear_depth_stencil` functions now take the image being cleared: it is now possible to clear and attach, but not store or resolve, an image
- `record_`-* methods now also provide a `Bindings` parameter to the recording closure
- `RayTracePipeline::group_handle` is now an associated function where previously it was a method
- Many types have been moved betwen modules in order to document things cleary

### Removed

- `attach_color` and `attach_depth_stencil` functions: replace with the `load_` and `store_` functions for color or depth/stencil attachments
- `device_api.rs` helper functions: create resources directly
- `run` stand-alone function: Use `EventLoop` directly.
- Various internal-only fields and other types within the `driver` module

### Fixed

- Depth/stencil attachment clear requests are properly handled in cases where the image used is transient

## [0.5.0] - 2022-09-17

### Added

- `LazyPool` resource pool which tries to find acceptable resources before creating new ones
- `instance_slice` function for acceleration structures
- `new_blas` and `new_tlas` helper functions for acceleration structure info
- Node-`_mut` functions for `PassRef`: enables clearer code patterns when building passes
- `rt_triangle.rs` example; similar to `triangle.rs` but uses a ray trace pipeline

### Changed

- `build_structure` and `update_structure` now take geometry info as a borrow instead of by value

### Deprecated

- `input::Typing` struct; it has use valid cases but is not within the scope of this crate

### Removed

- `prelude_arc`: Use `prelude` module instead
- `driver::BlendMode::Replace` and other camel-case constants; use screaming-snake versions, i.e. `driver::BlendMode::REPLACE`

### Fixed

- Windows platform: `EventLoop` no longer panics if the window is minimized

## [0.4.2] - 2022-06-28

### Added

- `create_from_slice` function for buffers

## [0.4.1] - 2022-06-24

### Added

- `update_structure` function for acceleration structures
- `group_handle` function for ray trace pipelines

## [0.4.0] - 2022-06-06

_See [#25](https://github.com/attackgoat/screen-13/pull/25) for migration details_

### Added

- Resources may now be bound using `Arc<T>` of `driver` smart pointers: _`Buffer`, `Image`, etc_

### Changed

- Resource state is now held in the `driver` smart pointers instead of the current graph

### Removed

- "Binding" types, such as `ImageBinding` and `ImageLeaseBinding`: _use `Arc<Image>` instead_
- Dependency on the `archery` crate; _see [rationale](https://github.com/attackgoat/screen-13/pull/24)_

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

[Unreleased]: https://github.com/attackgoat/screen-13/compare/v0.9.4...HEAD
[0.1.0]: https://crates.io/crates/screen-13/0.1.0
[0.2.0]: https://crates.io/crates/screen-13/0.2.0
[0.3.0]: https://crates.io/crates/screen-13/0.3.0
[0.3.1]: https://crates.io/crates/screen-13/0.3.1
[0.3.2]: https://crates.io/crates/screen-13/0.3.2
[0.4.0]: https://crates.io/crates/screen-13/0.4.0
[0.4.1]: https://crates.io/crates/screen-13/0.4.1
[0.4.2]: https://crates.io/crates/screen-13/0.4.2
[0.5.0]: https://crates.io/crates/screen-13/0.5.0
[0.6.0]: https://crates.io/crates/screen-13/0.6.0
[0.6.1]: https://crates.io/crates/screen-13/0.6.1
[0.6.2]: https://crates.io/crates/screen-13/0.6.2
[0.6.3]: https://crates.io/crates/screen-13/0.6.3
[0.6.4]: https://crates.io/crates/screen-13/0.6.4
[0.6.5]: https://crates.io/crates/screen-13/0.6.5
[0.7.0]: https://crates.io/crates/screen-13/0.7.0
[0.7.1]: https://crates.io/crates/screen-13/0.7.1
[0.8.0]: https://crates.io/crates/screen-13/0.8.0
[0.8.1]: https://crates.io/crates/screen-13/0.8.1
[0.9.0]: https://crates.io/crates/screen-13/0.9.0
[0.9.1]: https://crates.io/crates/screen-13/0.9.1
[0.9.2]: https://crates.io/crates/screen-13/0.9.2
[0.9.3]: https://crates.io/crates/screen-13/0.9.3
[0.9.4]: https://crates.io/crates/screen-13/0.9.4
