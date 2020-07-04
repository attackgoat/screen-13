#version 450

layout(location = 0) in vec2 v_position;

layout(location = 0) out vec2 f_position;

void main() {
    vec4 screen_position = vec4(v_position, 0, 1);
    f_position = screen_position.xy;
    gl_Position = screen_position;
}
