#include "blend.glsl"

vec3 blend_op(vec3 a, vec3 b) { return abs(a - b); }

#include "main.frag"
