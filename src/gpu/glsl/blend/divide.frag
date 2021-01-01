#version 450

#include "blend_decl.glsl"

vec3 blend_op(vec3 a, vec3 b) {
    return a / b;
}

#include "blend_fns.glsl"

void main() {
    write_blend();
}
