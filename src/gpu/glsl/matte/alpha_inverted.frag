#include "matte.glsl"

vec4 matte_op(vec4 image, vec4 matte) {
    float alpha_inv = min(image.a, 1 - matte.a);

    return vec4(alpha_inv * image.rgb, alpha_inv);
}

#include "main.frag"
