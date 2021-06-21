#version 450

#include "quad.glsl"

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 vertex_transform;
} push_constants;

void main() {
    texcoord_out = vertex_tex();
    gl_Position = push_constants.vertex_transform * vec4(vertex_pos(), 0, 1);
}
