void write_blend() {
    vec4 base = texture(base_sampler, base_uv) * push_constants.ab_inv;
    vec4 blend = texture(blend_sampler, blend_uv) * push_constants.ab;

    color = vec4(base.rgb * (1 - blend.a) + blend_op(base.rgb, blend.rgb),
                 base.a + blend.a);
}
