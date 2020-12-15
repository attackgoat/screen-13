#version 450

layout(set = 0, binding = 0) uniform sampler2D albedo_sampler;
layout(set = 0, binding = 1) uniform sampler2D material_sampler;
layout(set = 0, binding = 2) uniform sampler2D normal_sampler;

layout(location = 0) in vec3 normal_in; // TODO: USE!
layout(location = 1) in vec2 texcoord_in;

layout(location = 0) out vec4 albedo_out;
layout(location = 1) out vec2 material_out;
layout(location = 2) out vec3 normal_out;

void main() {
    // Color
    vec3 albedo = texture(albedo_sampler, texcoord_in).rgb;

    // Metalness + Roughness
    vec2 material = texture(material_sampler, texcoord_in).rg;

    // Surface normal perturbation
    vec3 normal = texture(normal_sampler, texcoord_in).rgb;

    albedo_out = vec4(albedo, 1.0);
    material_out = material;
    normal_out = normal;
}
