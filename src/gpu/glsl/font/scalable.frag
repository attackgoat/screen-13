#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 64) vec4 color;
} push_constants;

layout(location = 0) out vec4 color;

void main() {
    color = push_constants.color;
}
