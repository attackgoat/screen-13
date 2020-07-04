#version 450

const uint COLOR = 0;
const uint DEPTH = 2;

layout(push_constant) uniform PushConstants {
    layout(offset = 100) float material_id;
}
push_constants;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 texcoord;

layout(set = 0, binding = 0) uniform sampler2D gbuf[2];
layout(set = 0, binding = 1) uniform sampler2D diffuse_sampler;

layout(location = 0) out vec4 color;

void main() { color = texture(gbuf[COLOR], texcoord); }
