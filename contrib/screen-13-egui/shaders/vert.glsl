#version 450

layout(location = 0) in vec2 i_pos;
layout(location = 1) in vec2 i_uv;
layout(location = 2) in vec4 i_color;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec2 o_uv;

layout(push_constant) uniform PushConstants{
    vec2 screen_size;
};


void main(){
    gl_Position = vec4(
            2.0 * i_pos.x / screen_size.x - 1.0,
            2.0 * i_pos.y / screen_size.y - 1.0,
            0.0, 1.0);
    o_color = i_color;
    o_uv = i_uv;
}
