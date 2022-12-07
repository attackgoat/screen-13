#version 460 core

#define LIGHT_POSITION vec3(2.0, 4.0, 4.0)
#define LIGHT_RANGE 16.0

layout(binding = 2) uniform sampler2D texture_sampler_llr;

layout(location = 0) in vec3 world_position;
layout(location = 1) in vec3 world_normal;
layout(location = 2) in vec2 texture0;

layout(location = 0) out vec4 color_out;

void main() {
    color_out = texture(texture_sampler_llr, texture0);

    vec3 light_dir = LIGHT_POSITION - world_position.xyz;
    float light_dist = length(light_dir);

    if (light_dist < LIGHT_RANGE) {
        light_dir = normalize(light_dir);

        float lambertian = max(0.0, dot(world_normal, light_dir));
        float attenuation = max(0.0, min(1.0, light_dist / LIGHT_RANGE));
        attenuation = 1.0 - attenuation * attenuation;

        color_out.rgb *= vec3(lambertian * attenuation);
    }
}