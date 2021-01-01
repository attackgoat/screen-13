const vec3 one = vec3(1, 1, 1);

layout(push_constant) uniform PushConstants {
    layout(offset = 0) layout(offset = 64) float ab;
    layout(offset = 0) layout(offset = 68) float ab_inv;
} push_constants;

layout(location = 0) in vec2 base_uv;
layout(location = 1) in vec2 blend_uv;

layout(set = 0, binding = 1) uniform sampler2D base_sampler;
layout(set = 0, binding = 0) uniform sampler2D blend_sampler;

layout(location = 0) out vec4 color;
