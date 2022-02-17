// Some parts based on:
// https://www.cs.upc.edu/~virtual/G/1.%20Teoria/06.%20Textures/Tangent%20Space%20Calculation.pdf

void decode_model() {
    uint idx = 3 * gl_GlobalInvocationID.x;
    uint a_idx = read_idx(idx);
    uint b_idx = read_idx(idx + 1);
    uint c_idx = read_idx(idx + 2);

    VertexIn a = src_buf[a_idx];
    VertexIn b = src_buf[b_idx];
    VertexIn c = src_buf[c_idx];

    // Calculate the normal of the front face of this triangle
    vec3 ba = b.position - a.position;
    vec3 ca = c.position - a.position;
    vec3 normal = normalize(cross(ba, ca));

    // Calculate the four-component tangent (with handedness)
    vec2 s = b.texcoord - a.texcoord;
    vec2 t = c.texcoord - a.texcoord;
    float r = 1 / (s.x * t.y - t.x * s.y);
    vec3 s_dir = r * vec3((t.y * ba.x - s.y * ca.x),
                          (t.y * ba.y - s.y * ca.y),
                          (t.y * ba.z - s.y * ca.z));
    vec3 t_dir = r * vec3((s.x * ca.x - t.x * ba.x),
                          (s.x * ca.y - t.x * ba.y),
                          (s.x * ca.z - t.x * ba.z));
    vec4 tangent = vec4(
        normalize(s_dir - normal * dot(normal, s_dir)),
        dot(cross(normal, s_dir), t_dir) >= 0 ? 1 : -1
    );

    dst_buf[a_idx] = VertexOut(a.material, normal, a.position, tangent, a.texcoord);
    dst_buf[b_idx] = VertexOut(a.material, normal, b.position, tangent, b.texcoord);
    dst_buf[c_idx] = VertexOut(a.material, normal, c.position, tangent, c.texcoord);
}