#include "blend.glsl"

// TODO: Remove this cruft
//#define ChannelBlend_ColorBurn(A,B)  ((uint8)((B == 0) ? B:max(0, (255 - ((255
//- A) << 8 ) / B)))) #define ChannelBlend_ColorDodge(A,B) ((uint8)((B == 255) ?
//B:min(255, ((A << 8 ) / (255 - B))))) #define ChannelBlend_VividLight(A,B)
//((uint8)(B < 128)?ChannelBlend_ColorBurn(A,(2 *
//B)):ChannelBlend_ColorDodge(A,(2 * (B - 128)))) #define
//ChannelBlend_HardMix(A,B)    ((uint8)((ChannelBlend_VividLight(A,B) < 128) ?
//0:255))

vec3 blend_op(vec3 a, vec3 b) {
    vec3 result;
    result.r = b.r < 0.5 ? 2 * a.r * b.r : 1 - 2 * (1 - a.r) * (1 - b.r);
    result.g = b.g < 0.5 ? 2 * a.g * b.g : 1 - 2 * (1 - a.g) * (1 - b.g);
    result.b = b.b < 0.5 ? 2 * a.b * b.b : 1 - 2 * (1 - a.b) * (1 - b.b);
    return result;
}

#include "main.frag"
