Vertex read_vertex(uint idx) {
    float x = src_buf[idx];
    float y = src_buf[++idx];
    float z = src_buf[++idx];
    float u = src_buf[++idx];
    float v = src_buf[++idx];
    vec3 position = vec3(x, y, z);
    vec2 texcoord = vec2(u, v);

    return Vertex(position, texcoord);
}

void write_vertex(Vertex vertex, vec3 normal, vec4 tangent, uint idx) {
    dst_buf[idx] = vertex.position.x;
    dst_buf[++idx] = vertex.position.y;
    dst_buf[++idx] = vertex.position.z;
    dst_buf[++idx] = normal.x;
    dst_buf[++idx] = normal.y;
    dst_buf[++idx] = normal.z;
    dst_buf[++idx] = tangent.x;
    dst_buf[++idx] = tangent.y;
    dst_buf[++idx] = tangent.z;
    dst_buf[++idx] = tangent.w;
    dst_buf[++idx] = vertex.texcoord.x;
    dst_buf[++idx] = vertex.texcoord.y;
}

void calc_vertex_attrs() {
    uint idx = push_constants.offset + gl_GlobalInvocationID.x;
    uint a_idx = read_idx(idx);
    uint b_idx = read_idx(++idx);
    uint c_idx = read_idx(++idx);
    Vertex a = read_vertex(a_idx);
    Vertex b = read_vertex(b_idx);
    Vertex c = read_vertex(c_idx);

    // Calculate the normal of the front face of this triangle
    vec3 ba = b.position - a.position;
    vec3 ca = c.position - a.position;
    vec3 normal = normalize(cross(ba, ca));

    // Calculate the four-component tangent (with handedness)
    vec2 s = b.texcoord - a.texcoord;
    vec2 t = b.texcoord - a.texcoord;
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

    // The write mask tells us if we are allowed to write these vertices
    uint a_mask = 1 & write_mask[a_idx >> 5] >> a_idx % 32;
    uint b_mask = 1 & write_mask[b_idx >> 5] >> b_idx % 32;
    uint c_mask = 1 & write_mask[c_idx >> 5] >> c_idx % 32;

    if (a_mask != 0) {
        write_vertex(a, normal, tangent, a_idx);
    }

    if (b_mask != 0) {
        write_vertex(b, normal, tangent, b_idx);
    }

    if (c_mask != 0) {
        write_vertex(c, normal, tangent, c_idx);
    }
}
