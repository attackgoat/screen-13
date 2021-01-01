#version 450

#include "blend_decl.glsl"

void main() {
    vec4 base = texture(base_sampler, base_uv);
    vec4 blend = texture(blend_sampler, blend_uv);
    float inv_a = 1 - blend.a;

    float a = min(1.0, blend.a + base.a);
    vec3 rgb = blend.rgb + base.rgb * inv_a;

    color = vec4(rgb, a);
}
