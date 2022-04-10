# Getting Started with _Screen 13_

This guide is intended for developers who are new to _Screen 13_ and want a step-by-step introduction. For further details on these
topics refer to the online [documentation](https://docs.rs/screen-13).

All sample code in this guide is assuming a `use screen_13::prelude_arc::*`.

## Required Packages

_Linux (Debian-like)_:
- `sudo apt install cmake uuid-dev`

_Mac OS_:
- `brew install cmake`
- `brew install ossp-uuid`

_Windows_:
- TODO (works but I haven't gathered the requirements)

## Driver

The `driver` module provides a `Driver` struct which provides a ready-to-use Ash Vulkan instance/device/swapchain. The driver itself,
as well as any buffers, images, or other resources created from them, use shared reference tracking to keep pointers alive. _Screen 13_
uses a generic type parameter to determine what type of reference tracking to use: `std::rc::Rc` or `std::sync::Arc`.

Most programs will use the `screen_13::prelude_arc::*` prelude because it provides concrete type aliases which do not
have generic parameters for shared references.

Creating a driver uses a configuration struct to enable or disable features:

```rust
let driver = Driver::new(&my_winit_window, DriverConfig {
    debug: false, // logs validation layer messages true, and Vulkan SDK installed
    desired_swapchain_image_count: 3,
    presentation: true, // require operating system window swapchain support
    ray_tracing: true,  // require KHR ray tracing support
    sync_display: true, // v-sync
});
let driver = driver.expect("Oh no I don't support debug/presentation/ray_tracing if set!");
let device: Shared<Device> = driver.device;
let ffi_device: ash::Device = **device;
```

If you plan on doing things with "driver-level" smart pointers you will have access to the [Ash](https://github.com/MaikKlein/ash) function
pointers and such. In order to create smart pointers for Vulkan types you will need to use a _Screen 13_-provided `create`
function, such as this one for a compute shader pipeline:

```rust
let spirv_code: &[u8] = ...
let pipeline = ComputePipeline::create(&device, ComputePipelineInfo::new(spirv_code)).unwrap();
```

There is a bunch of stuff in the driver level which is used internally, and could be used to write other complex Vulkan programs,
but most of it is further handled "for you" by the render graph module. Most users will need only these driver-level types:

### `Buffer`

```rust
// To create
let mut buf = Buffer::create(&device, BufferInfo {
    size: 1024,
    usage: vk::BufferUsageFlags::TRANSFER_SRC,
    can_map: true,
}).unwrap();
assert_ne!(*buf, vk::Buffer::null());

// To fill with data (example)
let mapped_slice: &mut [u8] = Buffer::mapped_slice_mut(&mut buf);
mapped_slice[0] = 0xff;
mapped_slice[1] = 0xfe;
mapped_slice[2] = 0xff;
mapped_slice[3] = 0xfe;
```

### `Image`

`ImageInfo`, like all info structs in _Screen 13_, is a builder and has many options.

```rust
// To create
let (width, height) = (320, 200);
let format = vk::Format::R8G8B8A8_UNORM;
let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
let info = ImageInfo::new_2d(format, width, height).usage(usage);
let img = Image::create(&device, info).unwrap();
assert_ne!(*img, vk::Image::null());
```

### `GraphicPipeline` and `Shader`

Creating a graphic pipelines requires a list of shaders and the following metadata about the
GPU configuration:

```rust
#[derive(Clone, Default)]
pub struct GraphicPipelineInfo {
    pub blend: BlendMode,
    pub depth_stencil: Option<DepthStencilMode>,
    pub extra_descriptors: Option<DescriptorBindingMap>,
    pub samples: SampleCount,
    pub two_sided: bool,
    pub vertex_input: VertexInputMode,
}
```

All of the above parameters implement `Copy` and are simple enums except for `extra_descriptors`, which
is an optional map of bindings that is only used in conjuction with texture arrays that use dynamic specialization
constants to specify length.

```rust
let info = GraphicPipelineInfo::default();
let vertex = Shader::new_vertex(include_bytes!("vertex.spv"));
let fragment = Shader::new_fragment(include_bytes!("fragment.spv"));
let pipeline = GraphicPipeline::create(&device, info, [vertex, fragment]).unwrap();
```

### `RayTracePipeline`

Work in progress, most of this was straight-copied from Kajiya and needs to be worked on/updated with the
latest updates to ComputePipeline/GraphicPipeline/RenderGraph.

## Render Graph

The main attraction of _Screen 13_ has got to be the `RenderGraph` structure. The design of this code
originated with a combination of [`PassBuilder`](https://github.com/EmbarkStudios/kajiya/blob/main/crates/lib/kajiya-rg/src/pass_builder.rs)
and [`render_graph.cpp`](https://github.com/Themaister/Granite/blob/master/renderer/render_graph.cpp).

`RenderGraph` allows full control of the Vulkan pipeline while at the same time offering a downright
pleasant API. It does use generics quite a bit, which is great for performance and compile-time
checks but hard to document sometimes. The generated documentation is complete, albiet somewhat opaque.

`RenderGraph` instances are cheap and easy to use and are intended for one-time use. There are
some gotchas to be aware of:

- Shared references (buffers, images, etc.) will be 'kept alive' until `RenderGraph` is dropped
- Dropping a `RenderGraph` is harmless at all times

Let's start simple and create a `RenderGraph`:

```rust
// Calling new() simply allocates two Vec's - this is basically free
let mut graph = RenderGraph::new();
```

### Bindings

Before we can use anything on a graph, we need to know about "bindings". The purpose of a binding is
to track resource state before and after interacting with a render graph. The `Buffer` and `Image`
structs we created need an `Arc<>` and extra `usize` in order to track this state. A "Binding" provides
those.

```rust
let buf = BufferBinding::new(buf);
let img = ImageBinding::new(img);
```

_Note:_ You cannot clone a binding, and the enclosed resource cannot be taken out. You may access
a mutable borrow using `get_mut()` if no shared references are alive.

### Nodes

Bindings may be directly "bound" to a single render graph, and may be later unbound as well - although
that step is optional. During the time a binding is bound we refer to it as a "node". Bound nodes
may only be used with the graphs they were bound to. Nodes implement `Copy` to make using them
easier.

```rust
println!("{:?}", buf); // BufferBinding
println!("{:?}", img); // ImageBinding

// Bind our resources into opaque "usize" nodes
let buf = graph.bind_node(buf);
let img = graph.bind_node(img);

// The results have unique types!
println!("{:?}", buf); // BufferNode
println!("{:?}", img); // ImageNode

// Unbind "node" back into the "binding" so we can use it again
let buf = graph.unbind_node(buf);
let img = graph.unbind_node(img);

// Magically, they return to the correct types!
println!("{:?}", buf); // BufferBinding
println!("{:?}", img); // ImageBinding
```

_Note:_ Once unbound, a binding may be used immediately on other graphs or discarded. Later we will
discuss resolving a graph, and it does have an impact on the order our render graph passes execute,
so reusing a binding within the same frame is considered somewhat advanced as a use case. It is
however, safe to do.

_Note:_ Once unbound, the node struct is invalid and should be dropped. There is no compile-time
warning for this condition.

### Basic Operations

These operations may cause warnings in the Vulkan SDK performance validation layers. There are more
efficient ways to do these basic operations using compute and graphic passes, but there are times
when you might want to quickly clear an image, for instance:

```rust
let mut graph = RenderGraph::new();
let buf = graph.bind_node(some_buf_binding);
let img = graph.bind_node(some_image_binding);
let (r, g, b, a) = (1.0, 0.0, 1.0, 1.0);
graph
    .clear_color_image(img, r, g, b, a)
    .copy_buffer_to_image(buf, img);
```

Notice how the graph uses builder pattern functions allow additional uses of the graph after
submitting one command to it. This operates like pushing onto a vec, where all commands
are logically executed in order. In the above case `img` would be cleared to magenta and then
the image that `buf` contains would be written to `img` starting at the top left corner.

The basic operations are:

- `copy_buffer(src_buf_node, dst_buf_node)`
- `copy_buffer_to_image(buf_node, img_node)`
- `copy_image(src_img_node, dst_img_node)`
- `clear_color_image(img_node, r, g, b, a)`
- `fill_buffer(buf_node, data)`
- others todo!

Each of these operations offers function overloads similar to:

- `copy_image(src, dst)`
- `copy_image_region(src, dst, region)`
- `copy_image_regions(src, dst, regions)`

### Render Passes

For any operations not already defined as functions on `RenderGraph`, you will need to "record
a pass" to the graph which handles them. This is analogous to a single database transaction
and will be treated as one contiguous unit of work by the graph systems.

Adding a pass to a graph will return a structure which provides a number of useful functions.
For the "access functions", it's up to you to pick a specific access or accept the (workable)
default:

- `access_node(node, vk_sync::AccessType)` - Tells the graph you will be doing something specific
  to this node in the next execution
- `read_node(node)` - Tells the graph you will be generally reading a node in the next execution
- `write_node(node)` - Tells the graph you will be generally writing a node in the next execution

Main functions:

- `execute(fn)` - Chain a first or additional Vulkan command sequence onto this pass
- `submit_pass()` - Return to the `RenderPass` borrow for additional commands and passes (optional)

Example:

```rust
let mut graph = RenderGraph::new();
let buf_node = graph.bind_node(buf_binding);
let img_node = graph.bind_node(img_binding);
graph
    .record_pass("Do some Vulkan")
    .execute(move |device, cmd_buf, bindings| unsafe {
        // I always run first!
    })
    .read_node(buf_node)
    .write_node(img_node)
    .execute(move |device, cmd_buf, bindings| unsafe {
        // device is &ash::Device
        // cmd_buf is vk::CommandBuffer
        // bindings is a magical object you can index with a node and get the Vulkan resource out!
        let vk_buf: vk::Buffer = *bindings[buf_node];
        let vk_img: vk::Image = *bindings[img_node];
        ...
    });
```

_Note:_ If we later record additional passes onto this graph they will happen-after these two
executions.

Pipeline function:

- `bind_pipeline(pipeline)` - Returns a `PipelinePassRef` which allows you to use a strongly typed
  API specific to compute, graphic, or ray trace passes

#### Compute

The following example returns a render graph which runs the given SPIR-V code using push constants.

```rust
let shader = ComputePipelineInfo::new(spirv_code);
let pipeline = Shared::new(ComputePipeline::create(&device, shader).unwrap());

RenderGraph::new()
    .record_pass("My compute pass")
    .bind_pipeline(&pipeline)
    .push_constants(42u32)
    .dispatch(128, 1, 1) // This point is where the commands are logically executed
```

Let's exaimine that a bit:

```rust
let mut graph: RenderGraph = RenderGraph::new();
let pass: PipelinePassRef = graph
    .record("My compute pass")
    .bind_pipeline(&pipeline);

pass
    .push_constants(42u32)
    .dispatch(128, 1, 1)
```

It's still not all that useful, but we're seeing the types at least now. What we need to do is add
inputs and outputs. Compute, graphic, and ray trace `PipelinePassRef`-types all have these helpful
functions:

- `access_descriptor(descriptor, node, vk_sync::AccessType)`
- `access_descriptor_as(descriptor, node, vk_sync::AccessType, view_info)`
- `access_descriptor_as_subrange(descriptor, node, vk_sync::AccessType, view_info, subresource_range)`
- `read_descriptor` and `write_descriptor` with `_as` and `_as_subrange` functions

These functions work like the `access`, `read`, and `write` functions on `PassRef` however they
use shader descriptor bindings instead. You may specify the descriptor parameter as either:

- `2` - Means "use binding index 2"
- `(1, 2)` - Means "use descriptor set 1, binding index 2"
- `(1, 2, [42])` - Means, as above, except we're talking about the 42nd array element
- In case you don't care about descriptor set indexes (you use one) then just don't specify it ie `(2, [42])`

As for push constants, you can stick whatever you like in there and it will be sent to the correct
stages of the pipeline. Data must be `Copy`.

#### Graphics

Graphics operations look very similar to compute operations with the following differences:

- Fixed-function pipeline configuration data (your chosen drawing mode) is specified
- Multiple shader stages are used
- We have to understand framebuffers (but luckily you do not have to use them!)

Assuming we create a graphics pipeline as we did earlier, and we want to draw a quad or something:

```rust
let pipeline = ...
let mut graph = RenderGraph::new();
let mut cache = HashPool::new(&device);
let output = graph.bind_node(cache.lease(ImageInfo { ... }));

graph
    .record_pass("Simple quad (no vertex buffer)")
    .bind_pipeline(&pipeline)
    .clear_color(0)
    .store_color(0, output)
    .draw(|device, cmd_buf, bindings| unsafe {
        device.cmd_draw(cmd_buf, 6, 1, 0, 0);
    })
```

You can see that color attachment index `0` is cleared and stored into a new concept, a leased image.

Leases are fantastic: lease things and use them and forget about them. They're harmless to drop and simply
return to the cache lease pool. Leases may also be used transparently with other regular images in all
graph APIs. They do have different physical types but you will see that if you try to hold onto one.

Attachment functions:

- `attach_color(idx, image_node)` - Synonym for `load_color` + `store_color`
- `attach_depth_stencil(idx, image_node)`
- `clear_color(idx)` / `clear_color_value(idx, value)`
- `clear_depth_stencil(idx)` / `clear_depth_stencil_value(idx, value)`
- `load_color(idx, image_node)` (and `depth_stencil`)
- `resolve_color(idx, image_node)` (and `depth_stencil`)
- `store_color(idx, image_node)` (and `depth_stencil`)

_Note:_ The attachment functions also have `_as` overloads which allow specific views to be specified

Auxiallary functions:

- `set_depth_stencil(mode)` - Modify the depth/stencil state of this pass from the next execution forward
- `set_render_area(x, y, width, height)` - Set framebuffer render area for image attachments of this pass
- `set_viewport(x, y, width, height, depth_range)` / `set_scissor(x, y, width, height)` - Modify rendering viewport/scissor from the next execution forward

#### Ray Tracing

These functions are a work in progress - please help out if you feel so inclined!

### Resolving / Executing

After a full render graph has been built using one or more passes, it is time to execute the graph.

You may "resolve" a graph in order to submit it to the GPU for processing:

```rust
let mut graph = RenderGraph::new();
let mut cache = HashPool::new(&device);

// [build graph and do stuff, not shown here]

graph.resolve().submit(&mut cache).unwrap();
```

In the above example, a command buffer is retrieved from the cache and used to record and submit the actual GPU commands. Once finished
the command buffer is placed back into the cache, where it remains until all work has completed. After the work completes this command
buffer may be reused in other leases.

_Note:_ It is very important to note that if the `cache` instance we created above is dropped then the GPU will be forced to wait until
the work has completed, which may cause a stall.

For more advanced use cases, you may resolve a graph up to but not including a specific node and then later resolve either more or all of
the remaining work. For example:

```rust
// Initialization not shown: cache, graph, device, etc.

let cmd_buf = cache.lease(device.queue.family).unwrap();
let mut resolver = graph.resolve();

// This does not record any commands for "important node"
resolver.record_node_dependencies(&mut cache, &cmd_buf, important_node);

// [Your program does stuff here]

// Your application-specific logic determines it is time to record "important node"
resolver.record_node(&mut cache, &cmd_buf, important_node);

// You likely want to follow-up with this if anything was isolated from "important node" on the graph
resolver.record_unscheduled_passes(&mut cache, &cmd_buf);
```

#### Swapchain

The most common use case for a `RenderGraph` is likely to be swapchain usage, and so dedicated presenter types exist to
make this path easier. Further, an `EventLoop` type is available to make all of these types avilable quickly. Example:

```rust
let event_loop = EventLoop::new().build().unwrap();
let display = GraphicPresenter::new(&event_loop.device)?;
let mut cache = HashPool::new(&event_loop.device);
let mut img_binding = Some(cache.lease(ImageInfo { ... }).unwrap());

event_loop.run(|frame| {
    // Bind "img_binding" to graph
    let img_node = frame.render_graph.bind_node(img_binding.take().unwrap());

    // Record passes and do stuff to "img_node"
    frame.render_graph
        .record_pass("Do something with img_node")
        .bind_pipeline(&pipeline)
        ...

    // Record the swapchain presentation pass (draw "img_node" to screen!)
    display.present_image(frame.render_graph, img_node, frame.swapchain);

    // Unbind img from graph so we have it for the next frame
    img_binding = Some(frame.render_graph.unbind_node(img_node));
}).unwrap();
```

## Helpful tools

- [VulkanSDK](https://vulkan.lunarg.com/sdk/home) _(Required when calling `EventLoop::debug(true)`)_
- NVIDIA: [nvidia-smi](https://developer.nvidia.com/nvidia-system-management-interface)
- AMD: [RadeonTop](https://github.com/clbr/radeontop)
- [RenderDoc](https://renderdoc.org/)