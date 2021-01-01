#version 450

#include "blend_decl.glsl"

float hard_light(float a, float b) {
    if (b < 0.5) {
        return 2 * a * b;
    } else {
        return 1 - 2 * (1 - a) * (1 - b);
    }
}

vec3 blend_op(vec3 a, vec3 b) {
    vec3 result;
    result.r = hard_light(a.r, b.r);
    result.g = hard_light(a.g, b.g);
    result.b = hard_light(a.b, b.b);

    return result;
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
