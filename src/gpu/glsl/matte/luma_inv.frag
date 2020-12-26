#include "matte.glsl"
#include "../gamma.glsl"

vec4 matte_op(vec4 image, vec4 matte) {
    float luma = min(image.a, 1 - gray(matte.rgb));

    return vec4(luma * image.rgb, luma);
}

#include "main.frag"
