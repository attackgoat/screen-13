#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) float opacity;
} push_constants;

layout(location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler2D image;

layout(location = 0) out vec4 color;

void main() {
    color = texture(image, uv) * push_constants.opacity;
}
