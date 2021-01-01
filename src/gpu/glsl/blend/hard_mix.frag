#version 450

#include "blend_decl.glsl"

vec3 blend_op(vec3 a, vec3 b) {
    vec3 result;
    result.r = b.r < 0.5
        ? 2 * a.r * b.r
        : 1 - 2 * (1 - a.r) * (1 - b.r);
    result.g = b.g < 0.5
        ? 2 * a.g * b.g
        : 1 - 2 * (1 - a.g) * (1 - b.g);
    result.b = b.b < 0.5
        ? 2 * a.b * b.b
        : 1 - 2 * (1 - a.b) * (1 - b.b);

    return result;
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
