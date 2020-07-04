void main() {
    vec4 image = texture(image_sampler, image_uv);
    vec4 matte = texture(matte_sampler, matte_uv);

    color = matte_op(image, matte);
}