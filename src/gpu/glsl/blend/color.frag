#include "blend.glsl"
#include "../hsl.glsl"

vec3 blend_op(vec3 a, vec3 b) {
    a = rgb_to_hsl(a);
    b = rgb_to_hsl(b);

    return hsl_to_rgb(vec3(b.xy, a.z));
}

#include "main.frag"
