#version 450

const uint COLOR = 0;
const uint MATERIAL = 1;
const uint NORMAL = 2;
const uint POSITION_DEPTH = 3;

layout(constant_id = 0) const float NEAR_PLANE = 0.0;
layout(constant_id = 1) const float FAR_PLANE = 256.0;

layout(push_constant) uniform PushConstants {
    layout(offset = 100) float material_id;
}
push_constants;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 texcoord;

layout(set = 0, binding = 0) uniform sampler2D diffuse_sampler;

layout(location = 0) out vec4[4] gbuf;

float linear_depth() {
    return (2.0f * NEAR_PLANE * FAR_PLANE) /
           (FAR_PLANE + NEAR_PLANE -
            (gl_FragCoord.z * 2.0f - 1.0f) * (FAR_PLANE - NEAR_PLANE));
}

void main() {
    vec3 diffuse = texture(diffuse_sampler, texcoord).rgb;

    // Write color attachments to avoid undefined behaviour (validation error)
    gbuf[COLOR] = vec4(0.0f);

    // Material channels (basic color and ID)
    gbuf[MATERIAL].rgb = diffuse;
    gbuf[MATERIAL].a = push_constants.material_id;

    // Normal channel + unused
    gbuf[NORMAL].xyz = normal;
    gbuf[NORMAL].w = 0.0f;

    // Position channel + Linear depth channel
    gbuf[POSITION_DEPTH].xyz = position;
    gbuf[POSITION_DEPTH].w = linear_depth();
}
