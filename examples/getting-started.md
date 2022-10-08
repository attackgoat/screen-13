# Getting Started with _Screen 13_

This guide is intended for developers who are new to _Screen 13_ and want a step-by-step introduction. For further details on these
topics refer to the online [documentation](https://docs.rs/screen-13/latest/screen_13/).

## Required Packages

_Linux (Debian-like)_:
- `sudo apt install cmake uuid-dev libfontconfig-dev libssl-dev`

_Mac OS (10.15 or later)_:
- Xcode 12
- Python 2.7
- `brew install cmake ossp-uuid`

_Windows_:
- TODO (works but I haven't gathered the requirements)

## Documentation

Read the generated [documentation](https://docs.rs/screen-13/latest/screen_13/) online, or run the
following command locally:

```
cargo doc --open
```

## Changes

Stay informed of recent changes to _Screen 13_ using the
[change log](https://github.com/attackgoat/screen-13/blob/master/CHANGELOG.md) file.

## Helpful tools

- [VulkanSDK](https://vulkan.lunarg.com/sdk/home) _(Required when calling `EventLoop::debug(true)`)_
- NVIDIA: [nvidia-smi](https://developer.nvidia.com/nvidia-system-management-interface)
- AMD: [RadeonTop](https://github.com/clbr/radeontop)
- [RenderDoc](https://renderdoc.org/)