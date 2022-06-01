#version 450

layout(location = 0) in vec4 i_color;
layout(location = 1) in vec2 i_uv;

layout(location = 0) out vec4 o_color;

layout(binding = 0, set = 0) uniform sampler2D font_texture;

void main(){
    o_color = i_color * texture(font_texture, i_uv);
}
