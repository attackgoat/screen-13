#version 450

layout(location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler2D image;

layout(location = 0) out vec4 color;

void main() {
    color = texture(image, uv);
}
