#include "blend.glsl"
#include "../hsl.glsl"

vec3 blend_op(vec3 a, vec3 b) { return min(a, b); }

#include "main.frag"
