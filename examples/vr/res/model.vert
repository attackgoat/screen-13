#version 460 core
#extension GL_EXT_multiview : require

struct View {
    mat4 projection;
    mat4 view;
};

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 model;
} push_const;

layout(binding = 0) uniform Multiview {
    View views[2];
} multiview;

layout(location = 0) in vec3 model_Position;
layout(location = 1) in vec4 model_Tangent_encoded;
layout(location = 2) in vec3 model_Normal;
layout(location = 3) in vec2 model_TexCoord;

layout(location = 0) out vec3 world_Position;
layout(location = 1) out vec3 view_Position;
layout(location = 2) out vec3 view_Tangent;
layout(location = 3) out vec3 view_Bitangent;
layout(location = 4) out vec3 view_Normal;
layout(location = 5) out vec2 vk_TexCoord;

void main() {
    world_Position = (push_const.model * vec4(model_Position, 1.0)).xyz;
    mat4 model_View = multiview.views[gl_ViewIndex].view * push_const.model;

    view_Position = (multiview.views[gl_ViewIndex].view * vec4(world_Position, 1.0)).xyz;
    gl_Position = multiview.views[gl_ViewIndex].projection * multiview.views[gl_ViewIndex].view * push_const.model * vec4(model_Position, 1.0);

    // Decode the packed tangent and bitangent values
    float bitangent_Sign = model_Tangent_encoded.w;
    vec3 model_Tangent = model_Tangent_encoded.xyz;
    vec3 model_Bitangent = bitangent_Sign * cross(model_Normal, model_Tangent);

    view_Tangent = mat3(model_View) * model_Tangent;
    view_Bitangent = mat3(model_View) * model_Bitangent;
    view_Normal = mat3(model_View) * model_Normal;

    vk_TexCoord = model_TexCoord;
}
