#version 460 core

layout(push_constant) uniform PushConstants {
    layout(offset = 64) vec3 light_Position;
} push_const;

layout(binding = 1) uniform sampler2D normal_Sampler;
layout(binding = 2) uniform sampler2D occlusion_Sampler;

layout(location = 5) in vec2 model_TexCoord;

layout(location = 0) out vec4 vk_Color;

void main() {
    vec3 normal_Value = texture(normal_Sampler, model_TexCoord).rgb;
    float occlusion_Amount = texture(occlusion_Sampler, model_TexCoord).r;
    vk_Color = vec4(vec3(occlusion_Amount), 1);
}
