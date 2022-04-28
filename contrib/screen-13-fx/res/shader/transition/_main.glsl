void main() {
    vec2 size =  imageSize(dest_image);
    vec2 uv = vec2(gl_GlobalInvocationID.xy) / max(size.x, size.y);
    vec4 color = transition(uv);

    imageStore(dest_image, ivec2(gl_GlobalInvocationID.xy), color);
}
