#version 450

const float X[6] = {0, 1, 0, 1, 0, 1};
const float Y[6] = {0, 0, 1, 1, 1, 0};

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4x4 transform;
}
push_constants;

layout(location = 0) out vec2 texcoord_out;

vec2 get_texcoord() { return vec2(X[gl_VertexIndex], Y[gl_VertexIndex]); }
