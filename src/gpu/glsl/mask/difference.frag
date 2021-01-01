#version 450

#include "mask_decl.glsl"

float mask_op(float lhs, float rhs) {
    return abs(lhs - rhs);
}

#include "mask_fns.glsl"

void main() {
    write_mask();
}
