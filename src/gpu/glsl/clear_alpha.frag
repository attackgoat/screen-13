#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec3 clear;
} push_constants;

layout(location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler2D image;

layout(location = 0) out vec4 color;

void main() {
    vec4 blend = texture(image, uv);
    color = vec4(push_constants.clear * (1 - blend.a) + blend.rgb, 1);
}
