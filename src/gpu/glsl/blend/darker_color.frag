#version 450

#include "blend_decl.glsl"
#include "../hsl.glsl"

vec3 blend_op(vec3 a, vec3 b) {
    return vec3(min(a.x, b.x), min(a.y, b.y), min(a.z, b.z));
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
