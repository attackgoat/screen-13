#include "mask.glsl"

float mask_op(float lhs, float rhs) { return max(lhs, rhs); }

#include "main.frag"
