#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 transform;
} push_constants;

layout(location = 0) in vec2 position_texcoord_in;

layout(location = 0) out vec2 texcoord_out;
layout(location = 1) out vec2 texcoord_transform_out;

void main() {
    gl_Position = push_constants.transform * vec4(position_texcoord_in, 0, 1);
    texcoord_out = 0.5 + 0.5 * gl_Position.xy / gl_Position.ww;
    texcoord_transform_out = position_texcoord_in;
}
