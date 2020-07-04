#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) float time;
    layout(offset = 0) float glitch;
    layout(offset = 0) int width;
    layout(offset = 0) int height;
}
push_constants;

layout(location = 0) in vec2 f_texcoord;

layout(set = 0, binding = 0) uniform sampler2D ldr_sampler;
layout(set = 0, binding = 1) uniform sampler3D lut_sampler;
layout(set = 0, binding = 2) uniform sampler2D random_sampler;
layout(set = 0, binding = 3) uniform sampler2D vignette_sampler;

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

vec3 desaturate(vec3 color) {
    float s = (color.r + color.g + color.b) / 3;
    return vec3(s, s, s);
}

vec3 color_correction(vec3 color, sampler3D lut, int lut_size) {
    float scale = (float(lut_size) - 1.0) / float(lut_size);
    float offset = 1.0 / (2.0 * float(lut_size));

    return texture(lut, clamp(color, 0.0, 1.0) * scale + offset).rgb;
}

void main() {
    vec3 color = texture(ldr_sampler, f_texcoord).rgb;
    if (push_constants.glitch == 0) {
        p_color = color;
    } else {
        // THE STUFF IN THIS BRANCH SEEMS TO GO HAYWIRE EVERY OTHER RUN OR SO -
        // NOT SURE LOOKS LIKE UNINITIALIZED DATA?
        {
            vec2 first = mod(f_texcoord + color.rg * vec2(5.0, 5.11) +
                                 color.b * vec2(-5.41) +
                                 color.rg * push_constants.time * 0.05,
                             1.0);
            vec4 second = texture(
                random_sampler,
                mod(first / 512.0 *
                        vec2(push_constants.width, push_constants.height),
                    1.0));
            vec4 third = texture(random_sampler,
                                 mod(first * 0.2 + second.rg * 0.1 +
                                         color.gb * push_constants.time * 0.05,
                                     1.0));
            color =
                mix(color, 1.0 * desaturate(third.rgb), push_constants.glitch);
        }
        {
            vec3 vignette = texture(vignette_sampler, f_texcoord).rgb;
            color = color * mix(vignette, vec3(1.0, 1.0, 1.0), 0.0);
        }
        p_color = color_correction(color, lut_sampler, 64);
    }
}
