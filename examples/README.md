# _Screen 13_ Example Code

## Getting Started

A helpful [getting started](getting-started.md) guide is available which describes basic _Screen 13_
types and functions.

See the [README](../README.md) for more information.

## Example Code

Example | Instructions | Preview
 --- | --- | :---:
[cpu_readback.rs](cpu_readback.rs) | <pre>cargo run --example cpu_readback</pre> | _See console output_
[debugger.rs](debugger.rs) | <pre>cargo run --example debugger</pre> | _See console output_
[hello_world.rs](hello_world.rs) | <pre>cargo run --example hello_world</pre> | <image alt="Preview" src="../.github/img/hello_world.png" height=149 width=176>
[triangle.rs](triangle.rs) | <pre>cargo run --example triangle</pre> | <image alt="Preview" src="../.github/img/triangle.png" height=149 width=176>
[bindless.rs](bindless.rs) | <pre>cargo run --example bindless</pre> | <image alt="Preview" src="../.github/img/bindless.png" height=149 width=140>
[egui.rs](egui.rs) | <pre>cargo run --example egui</pre> | <image alt="Preview" src="../.github/img/egui.png" height=149 width=176>
[imgui.rs](imgui.rs) | <pre>cargo run --example imgui</pre> | <image alt="Preview" src="../.github/img/imgui.png" height=149 width=176>
[font_bmp.rs](font_bmp.rs) | <pre>cargo run --example font_bmp</pre> | <image alt="Preview" src="../.github/img/font_bmp.png" height=149 width=176>
[multipass.rs](multipass.rs) | <pre>cargo run --example multipass</pre> | <image alt="Preview" src="../.github/img/multipass.png" height=149 width=176>
[multithread.rs](multithread.rs) | <pre>cargo run --example multithread --release</pre> | <image alt="Preview" src="../.github/img/multithread.png" height=149 width=176>
[msaa.rs](msaa.rs) | <pre>cargo run --example msaa</pre> Multisample anti-aliasing | <image alt="Preview" src="../.github/img/msaa.png" height=149 width=176>
[rt_triangle.rs](rt_triangle.rs) | <pre>cargo run --example rt_triangle</pre> | <image alt="Preview" src="../.github/img/rt_triangle.png" height=149 width=176>
[ray_trace.rs](ray_trace.rs) | <pre>cargo run --example ray_trace</pre> | <image alt="Preview" src="../.github/img/ray_trace.png" height=149 width=176>
[vsm_omni.rs](vsm_omni.rs) | <pre>cargo run --example vsm_omni</pre> Variance shadow mapping for omni/point lights | <image alt="Preview" src="../.github/img/vsm_omni.png" height=149 width=176>
[transitions.rs](transitions.rs) | <pre>cargo run --example transitions</pre> | <image alt="Preview" src="../.github/img/transitions.png" height=149 width=176>
[skeletal-anim/](skeletal-anim/src/main.rs) | <pre>cargo run --manifest-path examples/skeletal-anim/Cargo.toml</pre> Skeletal mesh animation using GLTF | <image alt="Preview" src="../.github/img/skeletal-anim.png" height=149 width=176>
[shader-toy/](shader-toy/src/main.rs) | <pre>cargo run --manifest-path examples/shader-toy/Cargo.toml</pre> | <image alt="Preview" src="../.github/img/shader-toy.png" height=105 width=176>

## Additional Examples

The following packages offer examples for specific cases not listed here:

- [contrib/screen-13-hot](../contrib/screen-13-hot/examples/README.md): Shader pipeline hot-reload