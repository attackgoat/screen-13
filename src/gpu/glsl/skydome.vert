// Based on: https://github.com/kosua20/opengl-skydome/blob/master/skydome_vshader.glsl

#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 world_view_proj;
    layout(offset = 64) mat3 star_rotation;
}
push_constants;

layout(location = 0) in vec3 position_in;

layout(location = 0) out vec3 position_out;
layout(location = 1) out vec3 star_position_out;

void main() {
    position_out = position_in;
    star_position_out = push_constants.star_rotation * normalize(position_in);

    gl_Position = (push_constants.world_view_proj * vec4(position_in, 1)).xyww;
}
