void main() {
    float global_mask = texture(global_mask_sampler, uv).r;
    float local_mask = texture(local_mask_sampler, uv).r;

    color = vec4(mask_op(global_mask, local_mask));
}