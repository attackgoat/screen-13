#include "mask.glsl"

float mask_op(float lhs, float rhs) { return abs(lhs - rhs); }

#include "main.frag"
