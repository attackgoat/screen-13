#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec3 start;
    layout(offset = 16) vec3 end;
}
push_constants;

layout(location = 0) in vec2 f_position;

layout(location = 0) out vec3 g_buffer;

void main() {
    float blend = (f_position.y + 1.0) / 2.0;

    g_buffer = mix(push_constants.end, push_constants.start, blend);
}
