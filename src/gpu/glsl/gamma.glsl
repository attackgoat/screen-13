/* Source:
 * http://stackoverflow.com/questions/596216/formula-to-determine-brightness-of-rgb-color
 */

float inv_gam_sRGB(float c) {
    if (c <= 0.04045)
        return c / 12.92;
    else
        return pow((c + 0.055) / 1.055, 2.4);
}

float gam_sRGB(float v) {
    if (v <= 0.0031308)
        return 12.92 * v;
    else
        return 1.055 * pow(v, 1 / 2.4) - 0.055;
}

float gray(vec3 c) {
    return gam_sRGB(0.212655 * inv_gam_sRGB(c.r) +
                    0.715158 * inv_gam_sRGB(c.g) +
                    0.072187 * inv_gam_sRGB(c.b));
}
