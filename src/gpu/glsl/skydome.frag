// Based on https://github.com/kosua20/opengl-skydome/blob/master/skydome_fshader.glsl

#version 450

const float PI = 3.14159265;
const float TWO_PI = 2 * PI;

layout(push_constant) uniform PushConstants {
    layout(offset = 0) vec3 sun_normal;
    layout(offset = 12) float time;
    layout(offset = 16) float weather;
}
push_constants;

layout(set = 0, binding = 0) uniform sampler2D cloud1_sampler;
layout(set = 0, binding = 1) uniform sampler2D cloud2_sampler;
layout(set = 0, binding = 2) uniform sampler2D moon_sampler;
layout(set = 0, binding = 3) uniform sampler2D sun_sampler;
layout(set = 0, binding = 4) uniform sampler2D tint1_sampler;
layout(set = 0, binding = 5) uniform sampler2D tint2_sampler;

layout(location = 0) in vec3 position_in;
layout(location = 1) in vec3 star_position_in;

layout(location = 0) out vec3 color_out;

//---------NOISE GENERATION------------
//Noise generation based on a simple hash, to ensure that if a given point on the dome
//(after taking into account the rotation of the sky) is a star, it remains a star all night long
float Hash(float n) {
    return fract((1.0 + sin(n)) * 415.92653);
}

float Noise3d(vec3 x) {
    float xhash = Hash(round(400 * x.x) * 37.0);
    float yhash = Hash(round(400 * x.y) * 57.0);
    float zhash = Hash(round(400 * x.z) * 67.0);
    return fract(xhash + yhash + zhash);
}

void main() {
    vec3 pos_norm = normalize(position_in);
    float dist = dot(push_constants.sun_normal, pos_norm);

    //We read the tint texture according to the position of the sun and the weather factor
    vec3 color_wo_sun = texture(tint2_sampler, vec2((push_constants.sun_normal.y + 1.0) * 0.5, max(0.01, pos_norm.y))).rgb;
    vec3 color_w_sun = texture(tint1_sampler, vec2((push_constants.sun_normal.y + 1.0) * 0.5, max(0.01, pos_norm.y))).rgb;
    color_out = push_constants.weather * mix(color_wo_sun, color_w_sun, dist * 0.5 + 0.5);

    //Computing u and v for the clouds textures (spherical projection)
    float u = 0.5 + atan(pos_norm.z, pos_norm.x) / TWO_PI;
    float v = - 0.5 + asin(pos_norm.y) / PI;

    //Cloud color
    //color depending on the weather (shade of grey) *  (day or night) ?
    vec3 cloud_color = vec3(min(push_constants.weather * 3.0 * 0.5, 1.0))
        * (push_constants.sun_normal.y > 0 ? 0.95 : 0.95 + push_constants.sun_normal.y * 1.8);

    //Reading from the clouds maps
    //mixing according to the weather (1.0 -> clouds1 (sunny), 0.5 -> clouds2 (rainy))
    //+ time translation along the u-axis (horizontal) for the clouds movement
    float transparency = mix(texture(cloud2_sampler, vec2(u + push_constants.time, v)).r,
                             texture(cloud1_sampler, vec2(u + push_constants.time, v)).r,
                             (push_constants.weather - 0.5) * 2.0);

    // Stars
    if (push_constants.sun_normal.y < 0.1) {//Night or dawn
        float threshold = 0.99;
        //We generate a random value between 0 and 1
        float star_intensity = Noise3d(normalize(star_position_in));
        //And we apply a threshold to keep only the brightest areas
        if (star_intensity >= threshold) {
            //We compute the star intensity
            star_intensity = pow((star_intensity - threshold) / (1.0 - threshold), 6.0) * (-push_constants.sun_normal.y + 0.1);
            color_out += vec3(star_intensity);
        }
    }

    //Sun
    float radius = length(pos_norm - push_constants.sun_normal);
    if (radius < 0.05) {//We are in the area of the sky which is covered by the sun
        float time = clamp(push_constants.sun_normal.y, 0.01, 1);
        radius = radius / 0.05;
        if(radius < 1.0 - 0.001) {//< we need a small bias to avoid flickering on the border of the texture
            //We read the alpha value from a texture where x = radius and y=height in the sky (~time)
            vec4 sun_color = texture(sun_sampler, vec2(radius, time));
            color_out = mix(color_out, sun_color.rgb, sun_color.a);
        }
    }

    //Moon
    float radius_moon = length(pos_norm + push_constants.sun_normal);//the moon is at position -sun_pos
    if (radius_moon < 0.03) {//We are in the area of the sky which is covered by the moon
        //We define a local plane tangent to the skydome at -sun_norm
        //We work in model space (everything normalized)
        vec3 n1 = normalize(cross(-push_constants.sun_normal, vec3(0, 1, 0)));
        vec3 n2 = normalize(cross(-push_constants.sun_normal, n1));
        //We project pos_norm on this plane
        float x = dot(pos_norm, n1);
        float y = dot(pos_norm, n2);
        //x,y are two sine, ranging approx from 0 to sqrt(2)*0.03. We scale them to [-1,1], then we will translate to [0,1]
        float scale = 23.57 * 0.5;
        //we need a compensation term because we made projection on the plane and not on the real sphere + other approximations.
        float compensation = 1.4;
        //And we read in the texture of the moon. The projection we did previously allows us to have an undeformed moon
        //(for the sun we didn't care as there are no details on it)
        color_out = mix(color_out, texture(moon_sampler, vec2(x, y) * scale * compensation + vec2(0.5)).rgb, clamp(-push_constants.sun_normal.y * 3, 0, 1));
    }

    //Final mix
    //mixing with the cloud color allows us to hide things behind clouds (sun, stars, moon)
    color_out = mix(color_out, cloud_color, clamp((2 - push_constants.weather) * transparency, 0, 1));
}
