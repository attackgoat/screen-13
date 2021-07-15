#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 80) vec4 glyph_color;
} push_constants;

layout(set = 0, binding = 0) uniform sampler2D page;

layout(location = 0) in vec2 texcoord;

layout(location = 0) out vec4 color;

void main() {
    vec4 page_colors = vec4(texture(page, texcoord).rgb, 1);
    color = page_colors * push_constants.glyph_color;
}
