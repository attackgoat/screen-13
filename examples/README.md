# Screen 13 Example Code

This guide provides patterns and samples that you can use to gain a further understanding of the
features _Screen 13_ provides and how to use them effectively. Any developer using _Screen 13_ for
the first time should understand these concepts. For a complete reference refer to the
[documentation](https://docs.rs/screen-13).

This guide does not cover the
[`Screen`](https://docs.rs/screen-13/latest/screen_13/trait.Screen.html) trait, which is an
extension on these concepts to enable rendering to an operating system window. See the example code
for more information on those topics.

**_NOTE_**: The following examples present code which should be read together as one complete program.
For example, the first example creates a
[`Gpu`](https://docs.rs/screen-13/latest/screen_13/gpu/struct.Gpu.html) instance referred to by the
`gpu` binding. In later examples bindings with the same name should be created in a similar way to
the previous examples.

## Prerequisites

- Linux: _Vulkan_
  - Latest [Vulkan SDK](https://vulkan.lunarg.com/sdk/home): _for debugging layers, otherwise debug
    builds will segfault_
  - If you do not want to install the SDK you can force _Screen 13_ into release mode by adding this
    to your `Cargo.toml`:

    ```toml
    [profile.dev.package.screen-13]
    opt-level = 3
    ```

- TODO

## Graphics Processing Unit

The core feature offererd by _Screen 13_ is the automation of your GPU for the purposes of either 2D
or 3D realtime rendering, such as games or simulations. A display is not required, so you can use
_Screen 13_ from an off-screen/headless context such as during the processing of a webserver
response.

Most users will use _Screen 13_ with a regular operating system window or using a fullscreen video
mode, but for this guide we will use a headless context. A GPU can be constructed like this:

```rust
use screen_13::prelude_rc::*;

let gpu = Gpu::offscreen();
```

## Rendering Still Images

Still image rendering is accomplished using the canvas-like
[`Render`](https://docs.rs/screen-13/latest/screen_13/gpu/struct.Render.html) type, which allows you
to compose a graph of commands. The only required parameter is the final image dimensions in pixels.

```rust
let dims = (128u32, 128u32);
let mut render = gpu.render(dims);
```

### Caching

Issuing commands against a `Render` causes resources such as descriptor sets, graphics pipelines and
textures to be created. Although the details of these operations are hidden from _Screen 13_ users,
storage of such resources can be important. By default, _Screen 13_ maintains all caching
automatically.

In order to control the caching of resources a `Cache` type is available and can be used as a
parameter when constructing `Render` instances. Providing a `Cache` during `Render` construction
causes all sub-resources to be members of the given cache instance. This is most useful if you have
multiple rendering paths happening at the same time, such as two player camera viewpoints.

```rust
let cache = Default::default();
let mut render = gpu.render_with_cache(dims, &cache);
```

### Basic Commands

All rendering commands use a builder-style which requires a final `.record(...);` call in order to
submit batches to the underlying hardware. Individual rendering commands are logically composited in
the order they are recorded.

All rendering commands:

- _Must be_ recorded before starting new commands _on the same_ `Render`
- _Should be_ recorded eventually

Compiler errors and warnings are raised in these respective conditions so they are easy to avoid.

#### Clearing with solid colors

The most basic operation, like its predecessor [`CLS`](https://en.wikipedia.org/wiki/CLS_(command)),
simply fills a `Render` with the solid color.

Basic usage, fills `render` with black:

```rust
render.clear().record();
```

Unlike `CLS`, you are able to specify a color using the command builder pattern:

```rust
render.clear().with(CORNFLOWER_BLUE).record();
```

The above `render` binding now contains a 128x128 blue image, but we can't see it. The next step
helps with this.

#### Encoding images to disk

In order to save images as JPEGs an encode command is provided:

```rust
render.encode().with_quality(0.92).record("screenshot.jpg");
```

_NOTE_: When `render` is dropped the graphics hardware will be flushed to complete any disk writes.
In high-performance situations individual renders should be retained for enough time to allow the
graphics hardware to finish. See the example code for details on writing a `Screen` implementation
which handles this automatically.

#### Gradients [IN PROGRESS]

The interface for this functionality is in flux. Currently it looks like:

```rust
let start = (-10, -10);
let end = (100, 10);
let olive_drab: Color = (128, 128, 0).into();
let path = [(start, CORNFLOWER_BLUE), (end, olive_drab)];

render.gradient(path).record();
```

_NOTE_: Currently does not work properly

#### Render-to-Texture

`Render` instances are of course backed by native graphics API textures. In some cases you have to
"resolve" a render into its native texture, for example so that it may be used for image `Write`
operations (more on that later).

_NOTE_: Resolving a render does not cause any specific pipeline stalling or other such "wait"
operations. It merely re-orders the internal command list so that the underlying native graphics API
knows to complete the operations on the render before the operations that happen after this resolve.

Here's how it works:

```rust
let tex = gpu.resolve(render);
```

#### Copying between renders

More advanced image rendering might require copying the visual contents of one render onto another;
perhaps to create a feedback buffer of previous images.

Basic usage:

```rust
let foo = gpu.render((128, 64));
let bar = gpu.render((64, 128));

foo.clear().with(CORNFLOWER_BLUE).record();
bar.clear().record();

let bar_tex = gpu.resolve(bar);

foo.copy(&bar_tex).record();
```

`foo` now contains two 64x64 squares, the left is black and the right is blue.

## Loading content

For anything more advanced than colors and lines we'll need to address the topic of content and how
_Screen 13_ prefers to handle it. All assets are baked at design-time using a process
[described here](../README.md#Asset%20Baking). We'll describe the additional source files below.

### Fonts

For some `comic-sans.fnt` bitmapped font file (BMFont is supported) you might have this bitmap font
asset `.toml` file, `comic-sans.toml`:

```toml
[bitmap-font]
src = 'comic-sans.fnt'
```

Additionally you have a main project `.toml` file, `example.toml`:

```toml
[content]

[[content.group]]
assets = ['comic-sans.toml']
```

Following the asset baking process you should now have an asset `.pak` file, `example.pak`. In code,
the font would be loaded like so:

```rust
let mut pak = Pak::open("example.pak")?;
let comic_sans = gpu.read_font(&mut pak, "comic-sans");
```

#### Using fonts with render instances

Once an font has been loaded, it can be efficiently used with a `Render` instance. The basic usage
is:

```rust
let pos = (24.0, 10.0);
render.text(pos, MAGENTA).record(&comic_sans, "Hello, world!");
```

Additional command builder options include outline color and generalized matrix transform.

### Images

For some `cat.jpg` file you might have this asset bitmap `.toml` file, `cat.toml`:

```toml
[bitmap]
src = 'cat.jpeg'
```

Additionally, you must have added this file to the main project `.toml` file, `example.toml`.
Following the asset baking process you should now have an asset `.pak` file, `example.pak`. In code,
the image would be loaded like so:

```rust
let cat = gpu.read_bitmap(&mut pak, "cat");
```

#### Using images with render instances

Once an image has been loaded, it can be efficiently written onto a `Render` instance. The basic
usage is:

```rust
render.write().record(&mut [
    Write::position(&cat, (5.0, 5.0))
]);
```

_NOTE_: To write a `Render` to another render you must first resolve the source render, as we did
with `bar` earlier:

```rust
render.write().record(&mut [
    Write::position(&cat, (5.0, 5.0))
    Write::position(&bar_tex, (2.0, 4.0))
]);
```

Additional command builder options include image tiling/atlasing, image stretching, and more. Of
note are the numerous blending, matting, and masking modes available using `WriteMode`.

### 3D Models

For some `teapot.gltf` file you might have this model asset `.toml` file, `teapot.toml`:

```toml
[model]
src = 'teapot.gltf'
```

_NOTE_: Model files require values for the `POSITION` and `TEXTURE0` vertex semantics and must be
indexed.

Additionally you might have this material asset `.toml` file, `glossy_cat.toml`:

```toml
[material]
color = 'cat.toml'
metal_src = 'cat_metal.png'
normal = 'cat_normal.toml'
rough_src = 'cat_rough.png'
```

_NOTE_: The `cat_normal.toml` file is another bitmap asset file, which is not shown. The `metal_src`
and `rough_src` keys point to grayscale images which are the metalness/roughness material parameters.

Additionally, you must have added this file to the main project `.toml` file, `example.toml`.
Following the asset baking process you should now have an asset `.pak` file, `example.pak`. In code,
the image would be loaded like so:

```rust
let teapot = gpu.read_model(&mut pak, "teapot");
```

#### Physically Based Rendering Materials

Materials may be created easily:

```rust
// Loading new assets (notice use of IDs and how metalness/roughness have been merged for us)
let glossy_cat = pak.material("glossy_cat");
let dog = gpu.read_bitmap(&mut pak, "dog"); // Ignroing `glossy_cat.color`! 🙀
let metal_rough = gpu.read_bitmap_with_id(&mut pak, glossy_cat.metal_rough);
let normal = gpu.read_bitmap_with_id(&mut pak, glossy_cat.normal);

// Make a sharable material
let glossy_cat = Material {
    color: dog, // 🐕's 4 eva!
    metal_rough,
    normal,
};
```

#### Using models with render instances

Once a model has been loaded, it can be efficiently drawn onto a `Render` instance. The basic usage
is:

```rust
let camera = Perspective::default();
let transform = Mat4::identity();

render.draw().record(&camera, &mut [
    Draw::model(teapot, glossy, transform)
]);
```

Numerous additional command builder options include:

- Animation pose
- Lighting (point, rectangular, spot, and sun)
- Lines
- Predicated rendering by mesh name
- Skydome (sun, moon, and stars)

_NOTE_: Currently a deferred volumetric lighting based renderer is available - an additional forward
renderer is planned
