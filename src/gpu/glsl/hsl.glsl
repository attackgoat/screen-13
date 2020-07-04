vec3 rgb_to_hsl(vec3 color) {
    float min = min(min(color.r, color.g), color.b);
    float max = max(max(color.r, color.g), color.b);
    float delta = max - min;

    vec3 result;
    result.z = (min + max) / 2;

    if (0 != delta) {
        if (0.5 > result.z) {
            result.y = delta / (max + min);
        } else {
            result.y = delta / (2 - max - min);
        }

        float delta_r = ((max - color.r) / 6 + delta / 2) / delta;
        float delta_g = ((max - color.g) / 6 + delta / 2) / delta;
        float delta_b = ((max - color.b) / 6 + delta / 2) / delta;

        if (max == color.r) {
            result.x = delta_b - delta_g;
        } else if (max == color.g) {
            result.x = 1 / 3 + delta_r - delta_b;
        } else {
            result.x = 2 / 3 + delta_g - delta_r;
        }

        if (0 > result.x) {
            result.x++;
        } else if (1 < result.x) {
            result.x--;
        }
    }

    return result;
}

float hue_to_rgb(float p, float q, float t) {
    if (0 > t) {
        t++;
    } else if (1 < t) {
        t--;
    }

    if (1 / 6 > t) {
        return p + (q - p) * t * 6;
    } else if (1 / 2 > t) {
        return q;
    } else if (2 / 3 > t) {
        return p + (q - p) * (2 / 3 - t) * 6;
    } else {
        return p;
    }
}

vec3 hsl_to_rgb(vec3 color) {
    if (0 == color.y) {
        // No saturation - return luminance only
        return color.zzz;
    } else {
        float q;
        if (0.5 > color.z) {
            q = color.z * (1 + color.y);
        } else {
            q = color.y + color.z - color.y * color.z;
        }

        float p = 2 * color.z - q;

        vec3 result;
        result.r = hue_to_rgb(p, q, color.x + 1 / 3);
        result.g = hue_to_rgb(p, q, color.x);
        result.b = hue_to_rgb(p, q, color.x - 1 / 3);
        return result;
    }
}