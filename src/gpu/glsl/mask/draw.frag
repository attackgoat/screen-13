#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 8) float opacity;
} push_constants;

layout(location = 0) out vec4 color;

void main() { color = vec4(push_constants.opacity); }
