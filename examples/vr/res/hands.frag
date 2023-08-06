#version 460 core
#extension GL_EXT_multiview : require

#include "camera.glsl"

layout(binding = 0) uniform CameraBuffer {
    Camera cameras[2];
} camera_buf;

layout(binding = 1) uniform LightBuffer {
    vec3 light_Position;
} light_buf;

layout(binding = 2) uniform sampler2D diffuse_Sampler;
layout(binding = 3) uniform sampler2D normal_Sampler;
layout(binding = 4) uniform sampler2D occlusion_Sampler;

layout(location = 0) in vec3 world_Position;
layout(location = 1) in vec4 world_Tangent;
layout(location = 2) in vec3 world_Normal;
layout(location = 3) in vec2 frag_TexCoord;

layout(location = 0) out vec4 vk_Color;

void main() {
    Camera camera = camera_buf.cameras[gl_ViewIndex];

    vec3 tangent_Normal = texture(normal_Sampler, frag_TexCoord).rgb * 2.0 - 1.0;
    vec3 world_Bitangent = cross(world_Normal, world_Tangent.xyz) * world_Tangent.w;
    vec3 world_Normal = normalize(tangent_Normal.x * world_Tangent.xyz
                                + tangent_Normal.y * world_Bitangent
                                + tangent_Normal.z * world_Normal);

    vec3 diffuse_Color = texture(diffuse_Sampler, frag_TexCoord).rgb;
    vec3 light_Direction = normalize(light_buf.light_Position - world_Position);
    light_Direction =  vec3(0.0, -1.0, 0.0);
    float light_Amount = max(dot(world_Normal, light_Direction), 0.05);
 
    vec3 specular_Color = diffuse_Color;
    vec3 view_Direction = normalize(camera.position - world_Position);
    vec3 reflect_Direction = reflect(light_Direction, world_Normal);
    float specular_Amount = pow(max(dot(view_Direction, reflect_Direction), 0.0), 1.5);
    
    if (dot(light_Direction, world_Normal) < 0.0) {
        specular_Amount = 0.0;
    }

    float occlusion_Amount = texture(occlusion_Sampler, frag_TexCoord).r;

    vk_Color.rgb = diffuse_Color * light_Amount * occlusion_Amount
                 + specular_Color * specular_Amount;
    vk_Color.a = 1.0;

}
