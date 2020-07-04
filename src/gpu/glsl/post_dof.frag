#version 450

// layout (push_constant) uniform PushConstants {
// layout(offset = 0) int width;
// layout(offset = 0) int height;
//} push_constants;

layout(location = 0) in vec2 f_texcoord;

layout(set = 0, binding = 0) uniform sampler2D ldr_sampler;
// layout (set = 0, binding = 1) uniform sampler2D depth_sampler;
// layout (set = 0, binding = 2) uniform sampler2D random_sampler;

layout(location = 0) out vec3 p_color;

vec3 to_gamma(vec3 color) {
    vec3 ret;
    ret.r = pow(color.r, 2.2);
    ret.g = pow(color.g, 2.2);
    ret.b = pow(color.b, 2.2);
    return ret;
}

vec3 from_gamma(vec3 color) {
    vec3 ret;
    ret.r = pow(color.r, 1.0 / 2.2);
    ret.g = pow(color.g, 1.0 / 2.2);
    ret.b = pow(color.b, 1.0 / 2.2);
    return ret;
}

vec3 pow3(vec3 col, float exponent) {
    vec3 ret;
    ret.r = pow(col.r, exponent);
    ret.g = pow(col.g, exponent);
    ret.b = pow(col.b, exponent);
    return ret;
}

void main() {
    float bloom_amount = 5.0;
    float bloom_color = 3.0;

    vec4 bloom_s = texture(ldr_sampler, f_texcoord);
    vec3 bloom = bloom_amount * bloom_s.a * pow3(bloom_s.rgb, bloom_color);

    // float focal_depth = 0.0015;
    float focal_depth = 0.0;

    // gl_FragColor.rgb = bloom + bokeh_dof(width, height, ldr_sampler,
    // depth_texture, random_texture, gl_TexCoord[0].xy, focal_depth); pColor.rgb
    // = to_gamma(bloom + texture(ldr_sampler, fTexcoord).rgb);

    p_color = bloom_s.rgb;
}
