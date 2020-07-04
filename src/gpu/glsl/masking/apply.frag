#version 450

layout(location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler2D global_mask_sampler;
layout(set = 0, binding = 1) uniform sampler2D image_sampler;

layout(location = 0) out vec4 color;

void main() {
    float global_mask = texture(global_mask_sampler, uv).r;
    vec4 image = texture(image_sampler, uv);

    color = vec4(image.rgb * global_mask, global_mask);
}
