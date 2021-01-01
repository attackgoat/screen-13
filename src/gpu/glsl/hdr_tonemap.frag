#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) float exposure;
} push_constants;

layout(location = 0) in vec2 f_texcoord;

layout(set = 0, binding = 0) uniform sampler2D hdr_sampler;

layout(location = 0) out vec3 p_color;

#define A 0.15
#define B 0.50
#define C 0.10
#define D 0.20
#define E 0.02
#define F 0.30
#define W 11.2
#define WHITE_SCALE 1.379064247

vec3 filmic_tonemap(vec3 color) {
    return ((color * (A * color + C * B) + D * E) /
            (color * (A * color + B) + D * F)) -
           E / F;
}

vec3 reinhard_tonemap(vec3 color) { return color / (color + vec3(1)); }

void main() {
    vec3 color = texture(hdr_sampler, f_texcoord).rgb;

    p_color = filmic_tonemap(color.rgb * push_constants.exposure);
    // color.r = clamp(pow(color.r, 1 / 2.2), 0.0, 1.0);
    // color.g = clamp(pow(color.g, 1 / 2.2), 0.0, 1.0);
    // color.b = clamp(pow(color.b, 1 / 2.2), 0.0, 1.0);
}
