#version 460 core
#extension GL_EXT_multiview : require

#include "camera.glsl"

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 model;
    layout(offset = 64) mat4 model_inv_transpose;
} push_const;

layout(binding = 0) uniform CameraBuffer {
    Camera cameras[2];
} camera_buf;

layout(location = 0) in vec3 vert_Position;
layout(location = 1) in vec4 vert_Tangent;
layout(location = 2) in vec3 vert_Normal;
layout(location = 3) in vec2 vert_TexCoord;

layout(location = 0) out vec4 world_Position;
layout(location = 1) out vec4 world_Tangent;
layout(location = 2) out vec3 world_Normal;
layout(location = 3) out vec2 frag_TexCoord;

void main() {
    Camera camera = camera_buf.cameras[gl_ViewIndex];
    world_Position = push_const.model * vec4(vert_Position, 1.0);
    gl_Position = camera.projection * camera.view * world_Position;

    world_Tangent = vec4(normalize(mat3(push_const.model) * vert_Tangent.xyz), vert_Tangent.w);
    world_Normal = normalize(mat3(push_const.model_inv_transpose) * vert_Normal);

    frag_TexCoord = vert_TexCoord;
}
