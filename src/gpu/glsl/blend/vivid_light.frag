#version 450

#include "blend_decl.glsl"

float vivid_light(float a, float b) {
    if (b < 0.5) {
        return 1 - (1 - a) / b;
    } else {
        return a / (1 - b);
    }
}

vec3 blend_op(vec3 a, vec3 b) {
    vec3 result;
    result.r = vivid_light(a.r, b.r);
    result.g = vivid_light(a.g, b.g);
    result.b = vivid_light(a.b, b.b);

    return result;
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
