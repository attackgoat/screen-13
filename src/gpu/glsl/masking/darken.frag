#include "mask.glsl"

float mask_op(float lhs, float rhs) { return min(lhs, rhs); }

#include "main.frag"
