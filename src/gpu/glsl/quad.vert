#include "quad.glsl"

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4x4 vertex_transform;
}
push_constants;

void main() {
    texcoord_out = vertex();
    gl_Position = push_constants.vertex_transform * vec4(texcoord_out, 0, 1);
}
