#version 460 core

#include "../inc/color_space.glsl"

layout(location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler2D image_sampler_llr;

layout(location = 0) out vec4 color;

void main()
{
    vec3 image_sample = texture(image_sampler_llr, uv).rgb;
    image_sample = linear_to_srgb(image_sample);

    color = vec4(image_sample, 1.0);
}
