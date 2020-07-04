#include "blend.glsl"

vec3 blend_op(vec3 a, vec3 b) { return a / (one - b); }

#include "main.frag"
