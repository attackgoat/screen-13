<image alt="Preview" src="../../.github/img/shader-toy.png">

# Shader Toy Example

This example uses computational fluid dynamics to create an effect like spilled paint. The original
shader code comes from [Florian Berger](https://www.shadertoy.com/view/MsGSRd) and is attached to a
permissive [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/) license.

The implementation is presented as close as possible to the original usage on Shader Toy, but it
would not be recommended to use it directly - you probably want to use compute pipelines for things
like this. Also there are numerous unused descriptor bindings and push constant ranges which could
be removed; but those are standard things all Shader Toys require.

## Details

See the `build.rs` script: it packs the hefty (they are actually not hefty) images into a `.pak`
file. This makes it easier for the example to be run in other places, but of course all this is
overkill for this actual example. It also pre-compiles the shader code from GLSL to SPIR-V.

### Adding/Changing files

The `pak.toml` file references the images used in this example using a glob see line 6 in that file.

If we add a reference directly to that file the build script will pick up and pack the new file. If
we let the glob continue to reference files you might want to ask the build script to look again,
like so:

```bash
touch res/pak.toml
```

Now try building again and the newly added files should be packed and have bindings generated in the
Rust code. If any of those files change, the build script will automatically re-pack things.