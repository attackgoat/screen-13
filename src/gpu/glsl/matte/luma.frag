#version 450

#include "matte_decl.glsl"
#include "../gamma.glsl"

vec4 matte_op(vec4 image, vec4 matte) {
    float luma = min(image.a, gray(matte.rgb));

    return vec4(luma * image.rgb, luma);
}

#include "matte_fns.glsl"

void main() {
    write_matte();
}
