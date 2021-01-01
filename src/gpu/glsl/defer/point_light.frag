#version 450

#include "rg3d.glsl"

layout(push_constant) uniform PushConstants {
    layout(offset = 64) mat4 view_proj_inv;
    layout(offset = 128) vec3 camera_eye;
    layout(offset = 144) vec2 depth_dims_inv;
    layout(offset = 160) vec3 light_center;
    layout(offset = 172) float light_radius;
    layout(offset = 176) vec3 light_intensity;
} push_constants;

layout(set = 0, binding = 0) uniform sampler2D depth_sampler;
layout(set = 0, binding = 1) uniform sampler2D normal_sampler;

void main() {
    vec2 screen_uv = gl_FragCoord.xy * push_constants.depth_dims_inv;
    float surface_depth = texture(depth_sampler, screen_uv).r;
    vec3 surface_position = S_UnProject(vec3(screen_uv, surface_depth), push_constants.view_proj_inv);
    vec3 surface_normal = normalize(texture(normal_sampler, screen_uv).rgb);

    // TBlinnPhongContext ctx;
    // ctx.lightPosition = push_constants.light.center;
    // ctx.lightRadius = push_constants.light.radius;
    // ctx.fragmentNormal = surface_normal;
    // ctx.fragmentPosition = surface_position;
    // ctx.cameraPosition = cameraPosition;
    // ctx.specularPower = 255.0 * texture(normalTexture, texCoord).w;
    // TBlinnPhong lighting = S_BlinnPhong(ctx);*/
    discard;
}
