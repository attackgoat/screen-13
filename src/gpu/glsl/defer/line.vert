#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 transform;
} push_constants;

layout(location = 0) in vec3 position_in;
layout(location = 1) in vec4 color_in;

layout(location = 0) out vec3 color_out;

void main() {
    color_out = color_in.rgb * vec3(color_in.a);

    gl_Position = push_constants.transform * vec4(position_in, 1);
}
