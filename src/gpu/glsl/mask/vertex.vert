#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec2 mask_size;
}
push_constants;

layout(location = 0) in vec2 position;

void main() {
    gl_Position = vec4(2 * position / push_constants.mask_size - 1, 0, 1);
}
