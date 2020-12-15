#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 world_view_proj;
}
push_constants;

layout(location = 0) in vec3 position_in;

void main() {
    gl_Position = push_constants.world_view_proj * vec4(position_in, 1);
}
