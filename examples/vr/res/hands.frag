#version 460 core

layout(push_constant) uniform PushConstants {
    layout(offset = 64) vec3 light_Position;
} push_const;

layout(binding = 1) uniform sampler2D diffuse_Sampler;
layout(binding = 2) uniform sampler2D normal_Sampler;
layout(binding = 3) uniform sampler2D occlusion_Sampler;

layout(location = 0) in vec3 world_Position;
layout(location = 1) in vec3 view_Position;
layout(location = 2) in vec4 view_Tangent;
layout(location = 4) in vec3 view_Normal;
layout(location = 5) in vec2 model_TexCoord;

layout(location = 0) out vec4 vk_Color;

void main() {
    vec3 diffuse_Color = texture(diffuse_Sampler, model_TexCoord).rgb;
    vec3 normal_Value = texture(normal_Sampler, model_TexCoord).rgb * 2.0 - 1.0;
    float occlusion_Amount = texture(occlusion_Sampler, model_TexCoord).r;

    // vec3 view_LightPosition = transpose(mat3(view_Tangent, view_Bitangent, view_Normal)) * ;
    vec3 normal_vector = normalize(mat3(view_Tangent, view_Bitangent, view_Normal) * normal_Value); 
    vec3 light_vector = normalize(push_const.light_Position - view_Position); 
    float diffuse_term = max(0.0, dot(normal_vector, light_vector))
                       * max(0.0, dot(view_Normal, light_vector));

        vk_Color = vec4(  diffuse_term+0.1); 
        if( diffuse_term > 0.0 ) { 
          vec3 half_vector = normalize(normalize( -view_Position.xyz  ) 
      + light_vector); 
          float specular_term = pow( dot( half_vector, normal_vector ), 
      70.0 ); 
          vk_Color += vec4( specular_term ); 
        } 



    // vk_Color.rgb *= occlusion_Amount;
    // vk_Color.rgb *= diffuse_Color;
    vk_Color.a = 1.0;
}
