void main() {
    // Material channels (basic color and ID)
    vec4 material = subpassLoad(material_sampler);
    vec3 diffuse = material.rgb;
    uint material_id = uint(material.a * 255.0f);

    // Normal channel + unused
    vec3 normal = subpassLoad(normal_sampler).xyz;

    // Position channel + Linear depth channel
    vec4 position_depth = subpassLoad(position_depth_sampler);
    vec3 position = position_depth.xyz;
    float depth = position_depth.w;

    color.rgb = light(diffuse, material_id, normal, position, depth);
    color.a = 1.0f;
}
