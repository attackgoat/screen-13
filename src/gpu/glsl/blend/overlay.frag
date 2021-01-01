#version 450

#include "blend_decl.glsl"

float overlay(float a, float b) {
    if (a < 0.5) {
        return 2 * a * b;
    } else {
        return 1 - 2 * (1 - a) * (1 - b);
    }
}

vec3 blend_op(vec3 a, vec3 b) {
    vec3 result;
    result.r = overlay(a.r, b.r);
    result.g = overlay(a.g, b.g);
    result.b = overlay(a.b, b.b);

    return result;
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
