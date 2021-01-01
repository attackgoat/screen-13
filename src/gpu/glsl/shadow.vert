#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 light_space;
    layout(offset = 0) mat4 world;
} push_constants;

layout(location = 0) in vec3 v_position;

void main() {
    gl_Position = push_constants.light_space * push_constants.world *
                  vec4(v_position, 1.0);
}
