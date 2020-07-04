#include "matte.glsl"

vec4 matte_op(vec4 image, vec4 matte) {
    float alpha = min(image.a, matte.a);

    return vec4(alpha * image.rgb, alpha);
}

#include "main.frag"
