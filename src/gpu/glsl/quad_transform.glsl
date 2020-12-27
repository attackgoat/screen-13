#include "quad.glsl"

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec2 texcoord_offset;
    layout(offset = 8) vec2 texcoord_scale;
    layout(offset = 16) mat4 vertex_transform;
}
push_constants;
