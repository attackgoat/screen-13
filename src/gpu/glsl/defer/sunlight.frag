#version 450

void main() {}


// #include "gbuf.glsl"

// const uint SAMPLE_COUNT = 4;
// const float SHADOW_SIZE = 1024;
// const uint POISSON_DISK_LEN = 4;
// const vec2 POISSON_DISK[POISSON_DISK_LEN] =
//     vec2[](vec2(-0.94201624, -0.39906216) / SHADOW_SIZE,
//            vec2(0.94558609, -0.76890725) / SHADOW_SIZE,
//            vec2(-0.09418410, -0.92938870) / SHADOW_SIZE,
//            vec2(0.34495938, 0.29387760) / SHADOW_SIZE);

// layout(push_constant) uniform PushConstants {
//     layout(offset = 0) vec3 diffuse;
//     layout(offset = 12) float ambient;
//     layout(offset = 16) mat4 lightspace;
//     layout(offset = 80) vec3 normal_inv;
//     layout(offset = 92) float power;
// }
// push_constants;

// layout(set = 0, binding = 3) uniform sampler2D lightmap_sampler;
// layout(set = 0, binding = 4) uniform sampler2D random_sampler;

// vec3 light(vec3 diffuse, uint material_id, vec3 normal, vec3 position,
//            float depth) {
//     vec4 lightspace_position = push_constants.lightspace * vec4(position, 1);
//     vec3 lightspace_normal = (push_constants.lightspace * vec4(normal, 0)).xyz;
//     vec3 lightspace_coords =
//         (lightspace_position.xyz / lightspace_position.w) * 0.5 + 0.5;
//     float lit = 1.0;

//     int random_value = int(texture(random_sampler, texcoord).r * 255);

//     for (int i = 0; i < SAMPLE_COUNT; i++) {
//         float closest_depth =
//             texture(lightmap_sampler,
//                     lightspace_coords.xy +
//                         POISSON_DISK[(random_value + i) % POISSON_DISK_LEN])
//                 .r;
//         float bias = 0.0; // max(0.0005 * (1.0 - dot(light_space_normal,
//                           // push_constants.normal_inv)), 0.00005);
//         lit -= lightspace_coords.z + bias < closest_depth ? 0.0 : 0.25;
//     }

//     return diffuse * push_constants.ambient +
//            lit * max(0.0, dot(normal, push_constants.normal_inv)) *
//                push_constants.diffuse * push_constants.power;
// }

// #include "main.frag"
