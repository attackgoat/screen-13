#version 450

layout(location = 0) in vec2 position_texcoord_in;

layout(location = 0) out vec2 texcoord_out;

void main() {
    texcoord_out = position_texcoord_in;

    gl_Position = vec4((position_texcoord_in.xy - 0.5) * 2, 0, 1);
}
