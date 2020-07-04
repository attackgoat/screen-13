#version 450

layout(location = 0) in vec3 color_in;

layout(location = 0) out vec4 color_out;

void main() { color_out = vec4(color_in, 1); }