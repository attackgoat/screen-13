#version 450

layout(set = 0, binding = 0) uniform sampler2D color_sampler;
layout(set = 0, binding = 1) uniform sampler2D material_sampler;
layout(set = 0, binding = 2) uniform sampler2D normal_sampler;

layout(location = 0) in vec3 bitangent_in;
layout(location = 1) in vec3 normal_in;
layout(location = 2) in vec3 tangent_in;
layout(location = 3) in vec2 texcoord_in;

layout(location = 0) out vec4 color_metal_out;
layout(location = 1) out vec4 normal_rough_out;

void main() {
    vec3 color = texture(color_sampler, texcoord_in).rgb;
    vec2 material = texture(material_sampler, texcoord_in).rg;
    vec3 poly_normal = texture(normal_sampler, texcoord_in).rgb * 2 - 1;

    // Metalness/Roughness are channels of the material texture
    float metal = material.r;
    float rough = material.g;

    // Triangle normal is adjusted by the normal map
    vec3 normal = normalize(poly_normal.x * tangent_in
                          + poly_normal.y * bitangent_in
                          + poly_normal.z * normal_in);

    // Fill the geometry buffers
    color_metal_out = vec4(color, metal);
    normal_rough_out = vec4(normal, rough);
}
