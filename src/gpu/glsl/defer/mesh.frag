#version 450

layout(location = 0) in vec3 normal_in; // TODO: USE!
layout(location = 1) in vec2 texcoord_in;

layout(set = 0, binding = 0) uniform sampler2D albedo_sampler;
layout(set = 0, binding = 1) uniform sampler2D material_sampler;
layout(set = 0, binding = 2) uniform sampler2D normal_sampler;

layout(location = 0) out vec4 geom_buf_albedo_out;
layout(location = 1) out vec2 geom_buf_material_out;
layout(location = 2) out vec3 geom_buf_normal_out;

void main() {
    vec3 albedo = texture(albedo_sampler, texcoord_in).rgb;
    vec2 material = texture(material_sampler, texcoord_in).rg;
    vec3 normal = texture(normal_sampler, texcoord_in).rgb;

    geom_buf_albedo_out = vec4(albedo, 1.0f);
    geom_buf_material_out = material;
    geom_buf_normal_out = normal;
}
