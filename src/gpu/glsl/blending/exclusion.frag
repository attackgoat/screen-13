#include "blend.glsl"

vec3 blend_op(vec3 a, vec3 b) { return a + b - 2 * a * b; }

#include "main.frag"
