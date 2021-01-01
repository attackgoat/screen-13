#version 450

#define STRENGTH 1.0
#define BASE 0.0
#define RADIUS 0.00005
#define SAMPLES 6
#define TILE 10.0

layout(push_constant) uniform PushConstants {
    layout(offset = 0) float clip_near;
    layout(offset = 0) float clip_far;
    layout(offset = 0) int width;
    layout(offset = 0) int height;
} push_constants;

layout(location = 0) in vec2 f_texcoord;

layout(set = 0, binding = 0) uniform sampler2DMS normals_sampler;
layout(set = 0, binding = 1) uniform sampler2DMS depth_sampler;
layout(set = 0, binding = 2) uniform sampler2D random_sampler;

layout(location = 0) out float p_color;

float smoothstep_map(float x) { return x * x * (3.0 - 2.0 * x); }

float difference_occlusion(float difference) {
    difference = max(difference, 0.0);

    /* This is the depth difference at which the maximum occlusion happens */
    float target =
        RADIUS * (push_constants.clip_far - push_constants.clip_near);

    /* This is the length of the falloff after maximum depth difference is
     * reached */
    float falloff = 5.0;

    float dist = (1.0 / target) * abs(difference - target);
    if (difference > target) {
        dist *= (1.0 / falloff);
    }

    dist = clamp(dist, 0.0, 1.0);

    return smoothstep_map(1.0 - dist);
}

void main() {
    vec3 pixel = texture(random_sampler, f_texcoord).rgb;

    ivec2 f_texcoord_ms = ivec2(textureSize(depth_sampler) * f_texcoord);
    float depth = texelFetch(depth_sampler, f_texcoord_ms, 0).r;
    vec3 normal = texelFetch(normals_sampler, f_texcoord_ms, 0).rgb;

    vec3 random = abs(normal.x) * texture(random_sampler, pixel.yz * TILE).rgb +
                  abs(normal.y) * texture(random_sampler, pixel.xz * TILE).rgb +
                  abs(normal.z) * texture(random_sampler, pixel.xy * TILE).rgb;
    random = normalize(random * 2.0 - 1.0);

    vec3 position = vec3(f_texcoord, depth);
    float radius_depth = RADIUS / depth;

    vec3 ray0 = radius_depth * reflect(vec3(-0.00, 0.02, -0.03), random);
    vec3 ray1 = radius_depth * reflect(vec3(0.35, -0.04, 0.31), random);
    vec3 ray2 = radius_depth * reflect(vec3(0.66, -0.32, 0.53), random);
    vec3 ray3 = radius_depth * reflect(vec3(-0.04, -0.04, 0.01), random);
    vec3 ray4 = radius_depth * reflect(vec3(0.24, -0.22, 0.89), random);
    vec3 ray5 = radius_depth * reflect(vec3(-0.09, 0.10, -0.54), random);

    vec3 projected0 =
        position + sign(dot(ray0, normal)) * ray0 *
                       vec3(push_constants.width, push_constants.height, 0);
    vec3 projected1 =
        position + sign(dot(ray1, normal)) * ray1 *
                       vec3(push_constants.width, push_constants.height, 0);
    vec3 projected2 =
        position + sign(dot(ray2, normal)) * ray2 *
                       vec3(push_constants.width, push_constants.height, 0);
    vec3 projected3 =
        position + sign(dot(ray3, normal)) * ray3 *
                       vec3(push_constants.width, push_constants.height, 0);
    vec3 projected4 =
        position + sign(dot(ray4, normal)) * ray4 *
                       vec3(push_constants.width, push_constants.height, 0);
    vec3 projected5 =
        position + sign(dot(ray5, normal)) * ray5 *
                       vec3(push_constants.width, push_constants.height, 0);

    float occlusion = 0.0;
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected0.xy * textureSize(depth_sampler)), 0)
                    .r);
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected1.xy * textureSize(depth_sampler)), 0)
                    .r);
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected2.xy * textureSize(depth_sampler)), 0)
                    .r);
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected3.xy * textureSize(depth_sampler)), 0)
                    .r);
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected4.xy * textureSize(depth_sampler)), 0)
                    .r);
    occlusion += difference_occlusion(
        depth - texelFetch(depth_sampler,
                           ivec2(projected5.xy * textureSize(depth_sampler)), 0)
                    .r);

    float ao = STRENGTH * occlusion * (1.0 / float(SAMPLES));
    p_color = 1.0 - clamp(ao + BASE, 0.0, 1.0);
}
