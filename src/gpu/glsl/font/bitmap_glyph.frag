#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 80) vec4 color;
} push_constants;

layout(set = 0, binding = 0) uniform sampler2D page;

layout(location = 0) in vec2 texcoord;

layout(location = 0) out vec4 color;

void main() {
    color = texture(page, texcoord).rrrr * push_constants.color;
}
