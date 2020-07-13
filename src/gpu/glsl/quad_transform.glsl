#version 450

const float X[6] = {0, 1, 0, 1, 0, 1};
const float Y[6] = {0, 0, 1, 1, 1, 0};

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec2 texcoord_offset;
    layout(offset = 8) vec2 texcoord_scale;
    layout(offset = 16) mat4x4 vertex_transform;
}
push_constants;

layout(location = 0) out vec2 texcoord_out;

// Returns the quad billboard coordinate for the current vertex. This quad is placed at (0,0) and evenly textures to (1,1).
vec2 vertex() { return vec2(X[gl_VertexIndex], Y[gl_VertexIndex]); }
