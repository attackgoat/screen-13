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
pointers and such.

### Creating `vk::Buffer` resources

Creating and filling a buffer is easy:

```rust
let mut some_buf = Buffer::create(
    &device,
    BufferInfo {
        size: 1024,
        usage: vk::BufferUsageFlags::TRANSFER_SRC,
        can_map: true,
    },
)?;

let data = Buffer::mapped_slice_mut(&mut some_buf);
data[0..4].copy_from_slice(&[0xff, 0xfe, 0xff, 0xfe]);

debug_assert_ne!(*some_buf, vk::Buffer::null());
debug_assert_eq!(some_buf.info.size, 1024);
```

### Creating `vk::Image` Resources

The full set of image options is available using the `ImageInfo` builder-pattern functions. A
typical image:

```rust
let (width, height) = (4096, 2184);
let some_image = Image::create(
    &device,
    ImageInfo::new_2d(
        vk::Format::R8G8B8A8_UNORM,
        width,
        height,
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::INPUT_ATTACHMENT
            | vk::ImageUsageFlags::TRANSIENT,
    ),
);

debug_assert_ne!(*some_image, vk::Image::null());
debug_assert!(some_image.info.usage.contains(vk::ImageUsageFlags::INPUT_ATTACHMENT));
```

### Creating compute `vk::Pipeline` resources

All pipelines support additional builder-pattern functions and specialized constants:

```rust
use inline_spirv::inline_spirv; // Provide SPIR-V using your choice of compiler

let comp_pipeline = ComputePipeline::create(
    &device,
    inline_spirv!(
        r#"
        #version 460 core
    
        layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
    
        void main() {
            // Incoming workgroup!! 🚚🚚🚚🚚🚚🚚!
        }
        "#,
        comp
    )
    .as_slice(),
)?;
```

### Creating graphic `vk::Pipeline` resources

Here we specify the full `GraphicPipelineInfo`, but you may provide `Default::default()` instead:

```rust
let info = GraphicPipelineInfo {
    blend: BlendMode::Replace,
    cull_mode: vk::CullModeFlags::BACK,
    depth_stencil: Some(DepthStencilMode {
        back: StencilMode::Noop,
        bounds_test: false,
        compare_op: vk::CompareOp::NEVER,
        depth_test: false,
        depth_write: false,
        front: StencilMode::Noop,
        min: OrderedFloat(0.0f32),
        max: OrderedFloat(1.0f32),
        stencil_test: false,
    }),
    front_face: vk::FrontFace::CLOCKWISE,
    name: Some("A name for debug purposes".to_owned()),
    polygon_mode: vk::PolygonMode::FILL,
    samples: SampleCount::X8,
    two_sided: false,
};

let gfx_pipeline = GraphicPipeline::create(
    &device,
    info,
    [
        Shader::new_vertex(
            inline_spirv!(
                r#"
                #version 460 core

                // Add descriptor bindings: buffers, images, inputs, etc.
                // Code is reflected automatically to wire things up

                void main() { /* 💎 */ }
                "#,
                vert
            )
            .as_slice(),
        ),
        Shader::new_fragment(
            inline_spirv!(
                r#"
                #version 460 core

                void main() { /* 🎨 */ }
                "#,
                frag
            )
            .as_slice(),
        ),
    ],
);
```

## Render Graph

The main attraction of _Screen 13_ has got to be the `RenderGraph` structure. The design of this code
originated with a combination of [`PassBuilder`](https://github.com/EmbarkStudios/kajiya/blob/main/crates/lib/kajiya-rg/src/pass_builder.rs)
and [`render_graph.cpp`](https://github.com/Themaister/Granite/blob/master/renderer/render_graph.cpp).

`RenderGraph` allows full control of the Vulkan pipeline while at the same time offering a downright
pleasant API. It does use generics quite a bit, which is great for performance and compile-time
checks but hard to document sometimes. The generated documentation is complete, albiet somewhat opaque.

`RenderGraph` instances are cheap and easy to use and are intended for one-time use. The design
principles are:

- Safe API abstraction for `vk::CommandBuffer`
- Compilation implies valid and optimal Vulkan usage
- Optimized for performance (_not code size_)

There are some caveats - Vulkan is full of parameters of various types and _Screen 13_ cannot prevent
these cases:

- SPIR-V code, `u32`'s, `&[u8]`'s, or other arguments go unchecked: _use validation layers!_
- Runtime panic if you declare illogical shader operations: _easy: follow `debug_assert!` advice_
- Runtime panic if you forget to declare access to nodes: _easy: explained further down_

Other notes:

- There is no "manager" type: a graph is just data your program decides things like thread model
- Shared references (buffers, images, etc.) will be 'kept alive' until `RenderGraph` is dropped
- Dropping a `RenderGraph` is harmless at all times (_except for a potential stall: see below_)

Let's start simple and create a `RenderGraph`:

```rust
// Calling new() simply allocates two Vec's - this is basically free
let mut graph = RenderGraph::new();
```

This graph may now have basic operations (like copy or clear without any shader code) and shader
passes recorded into it. After an entire frame worth of rendering operations have been recorded
the entire batch will processed, in efficient chunks as needed, in order to present to the display.

The entire process of construction and resolution of a render graph happens at the discretion of
your program and is highly optimized.

Each graph may use a given buffer or image multiple times, some as inputs and some as outputs. In
each case an optimal command submission order will be produced which executes the correct shaders
using minimal pipeline barriers.

Some notes about the awesome render pass optimization which was _totally stolen_ from [Granite](https://github.com/Themaister/Granite):

- Scheduling: passes are submitted to the Vulkan API using batches designed for low-latency
- Re-ordering: passes are shuffled using a hueristic which gives the GPU more time to complete work
- Merging: compatible passes are merged into dynamic subpasses when it is more efficient (_on-tile rendering_)
- Aliasing: resources and pipelines are optimized to emit minimal barriers per unit of work (_max one, typically zero_)

`RenderGraph` provides all of this with the expectation that construction and submission of each
graph should complete within ~250 μs. This is currently true for known-size graphs and I'm thinking
about how to bench this in the future.

### Bindings

Before we can use anything on a graph, we need to know about "bindings". The purpose of a binding is
to track resource state before and after interacting with a render graph. The `Buffer` and `Image`
structs we created need an `Arc<>` and extra `usize` in order to track this state. A "Binding" provides
those.

```rust
let buffer = BufferBinding::new(buffer);
let image = ImageBinding::new(image);
```

_Note:_ You cannot clone a binding, and the enclosed resource cannot be taken out. You may access
a mutable borrow using `get_mut()` if no shared references are alive.

### Nodes

Bindings may be directly "bound" to a single render graph, and may be later unbound as well - although
that step is optional. During the time a binding is bound we refer to it as a "node". Bound nodes
may only be used with the graphs they were bound to. Nodes implement `Copy` to make using them
easier.

```rust
println!("{:?}", buffer); // BufferBinding
println!("{:?}", image); // ImageBinding

// Bind our resources into opaque "usize" nodes
let buffer = graph.bind_node(buffer);
let image = graph.bind_node(image);

// The results have unique types!
println!("{:?}", buffer); // BufferNode
println!("{:?}", image); // ImageNode

// Unbind "node" back into the "binding" so we can use it again
let buffer = graph.unbind_node(buffer);
let image = graph.unbind_node(image);

// Magically, they return to the correct types!
println!("{:?}", buffer); // BufferBinding
println!("{:?}", image); // ImageBinding
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
let buffer = graph.bind_node(some_buffer_binding);
let image = graph.bind_node(some_image_binding);
let (r, g, b, a) = (1.0, 0.0, 1.0, 1.0);
graph
    .clear_color_image(image, r, g, b, a)
    .copy_buffer_to_image(buffer, image);
```

Notice how the builder pattern functions allow additional uses of the graph after
submitting the first command. This operates like pushing onto a vec, where all commands
are logically executed in order. In the above case `image` would be cleared to magenta and then
the image that `buffer` contains would be written to `image` starting at the top left corner.

The basic operations are:

- `copy_buffer(src_buffer_node, dst_buffer_node)`
- `copy_buffer_to_image(buf_node, image_node)`
- `copy_image(src_image_node, dst_image_node)`
- `clear_color_image(image_node, r, g, b, a)`
- `fill_buffer(buffer_node, data)`
- _etc_

Each of these operations offers function overloads similar to:

- `copy_image(src, dst)`
- `copy_image_region(src, dst, region)`
- `copy_image_regions(src, dst, regions)`

### Render Passes

For any operations not already defined as functions on `RenderGraph`, you will need to "record
a pass" to the graph which handles them. This is analogous to a single database transaction
and will be treated as one contiguous unit of work by the graph resolver.

Adding a pass to a graph will return a structure which provides a number of useful functions.
The primary set of functions handle vulkan synchronization and use prefixes of `access_`,
`read_`, or `write_`. For each resource used in a compute, subpass, ray trace, or general
command buffer you must call an access function. Generally choose a `read` or `write` function
unless you want to be most efficient.

- `access_node(node, vk_sync::AccessType)` - Tells the graph you will be doing something specific
  to this node in the next execution
- `read_node(node)` - Tells the graph you will be generally reading a node in the next execution
- `write_node(node)` - Tells the graph you will be generally writing a node in the next execution

_INFO:_ If you forget to declare access you will hit a very helpful `debug_assert!`.

Main functions:

- `record_cmd_buf(fn)` - Chain a first or additional Vulkan command sequence onto this pass
- `submit_pass()` - Return to the `RenderPass` borrow for additional commands and passes (optional)

Example:

```rust
let mut graph = RenderGraph::new();
let buffer_node = graph.bind_node(buffer_binding);
let image_node = graph.bind_node(image_binding);
graph
    .begin_pass("Do some Vulkan")
    .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
        // I always run first!
    })
    .read_node(buffer_node) // <-- These two functions, read_node/write_node, completely
    .write_node(image_node) //     handle vulkan synchronization. You are free to READ/WRITE below!
    .record_cmd_buf(move |device, cmd_buf, bindings| unsafe {
        // device is &ash::Device
        // cmd_buf is vk::CommandBuffer
        // bindings is a magical object you can index with a node and get the Vulkan resource out!
        let vk_buffer: vk::Buffer = *bindings[buffer_node];
        let vk_image: vk::Image = *bindings[image_node];
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
    .begin_pass("My compute pass")
    .bind_pipeline(&pipeline)
    .record_compute(|compute| {
        // This FnOnce is where the commands are executed
        // (Make this a move-closure and Send it 'static things if you want!)
        compute.push_constants(42u32)
               .dispatch(128, 1, 1);
        ...
    })
```

Let's exaimine that a bit:

```rust
let mut graph: RenderGraph = RenderGraph::new();
let pass: PassRef = graph.record("My compute pass");
let pass: PipelinePassRef = pass.bind_pipeline(&pipeline);
pass.record_compute(|compute| {
    // Mutable builder pattern so this works too
    compute.push_constants(42u32);
    compute.dispatch(128, 1, 1);
    ...
})
```

It's still not all that useful, but we're seeing the types at least now. What we need to do is add
inputs and outputs. Compute, graphic, and ray trace `PipelinePassRef`-types all have these helpful
functions:

- `access_descriptor(descriptor, node, vk_sync::AccessType)`
- `access_descriptor_as(descriptor, node, vk_sync::AccessType, view_info)`
- `access_descriptor_as_subrange(descriptor, node, vk_sync::AccessType, view_info, subresource)`
- `read_descriptor` and `write_descriptor` with `_as` and `_as_subrange` functions

Where:

- `node` is any type of buffer or image node
- `view_info` is an `ImageViewInfo { .. }` for images or `Range<u64>` for buffers
- `subresource` is an `ImageSubresource { .. }` for images or `Range<u64>` for buffers
- `descriptor` is a GLSL or HLSL shader binding point, as described below

These functions work like the `access`, `read`, and `write` functions on `PassRef`/`PipelinePassRef`
however they use shader descriptor bindings instead. You may specify the descriptor parameter as either:

- `2` - Means "use binding index 2"
- `(1, 2)` - Means "use descriptor set 1, binding index 2"
- `(1, 2, [42])` - Means, as above, except we're talking about the 42nd array element
- In case you don't care about descriptor set indexes (you use one) then just don't specify it ie `(2, [42])`

As for push constants, you can stick whatever you like in there and it will be sent to the correct
stages of the pipeline. Data must be `Copy`. Beware of device limits!

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
    .begin_pass("Simple quad (no vertex buffer)")
    .bind_pipeline(&pipeline)
    .clear_color(0)
    .store_color(0, output)
    .record_subpass(|subpass| {
        subpass.draw(6, 1, 0, 0);
    })
```

You can see that color attachment index `0` is cleared and stored into a new concept, a leased image.

Leases are fantastic: lease things and use them and forget about them. They're harmless to drop and simply
return to the cache lease pool. Leases may also be used transparently with other regular images in all
graph APIs. They do have different physical types but you will see that if you try to hold onto one.

_NOTE:_ The leasing API advanced from `screen-13` v0.1 to v0.2, but not quite as much as the driver/graph
layers. Expect to see changes and features like different cache pool types/strategies - and please PR
what you think might work!

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
- `record_subpass(fn)` - Allows you to do some graphic work; the callback has lots of functions for relevant vulkan APIs.

Image samplers:

By default, `Screen 13` will use "linear repeat-mode" samplers unless a special suffix appears as part of the
name within GLSL or HLSL shader code. The `_sampler_123` suffix should be used where `1`, `2`, and `2` are replaced with:

1. `l` for `LINEAR` texel filtering (default) or `n` for `NEAREST`
2. `l` (default) or `n`, as above, but for mipmap filtering
3. Addressing mode where:
  - `b` is `CLAMP_TO_BORDER`
  - `e` is `CLAMP_TO_EDGE`
  - `m` is `MIRRORED_REPEAT`
  - `r` is `REPEAT`

For example, the following sampler named `pages_sampler_nnr` specifies nearest texel/mipmap modes and repeat addressing:

```glsl
layout(set = 0, binding = 0) uniform sampler2D pages_sampler_nnr[NUM_PAGES];
```

Vertex input:

Optional name suffixes are used in the same way with vertex input as with image samplers. The additional
attribution of your shader code is again optional but may help in a few scenarios:

- Per-instance vertex rate data
- Multiple vertex buffer binding indexes

The data for vertex input is assumed to be per-vertex and bound to vertex buffer binding index zero. Add `_ibindX` for per-instance data, or the matching `_vbindX` for per-vertex data where `X` is replaced with the vertex buffer binding index in each case.

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
the work has completed, which may cause the current thread to stall for a couple milliseconds.

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
        .begin_pass("Do something with img_node")
        .bind_pipeline(&pipeline)
        ...

    // Record the swapchain presentation pass (draw "img_node" to screen!)
    // (See the present pass recorded in ~20 lines: https://bit.ly/3L6cn8U!)
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