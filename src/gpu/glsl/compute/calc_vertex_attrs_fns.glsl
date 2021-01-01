// Some parts based on: https://www.cs.upc.edu/~virtual/G/1.%20Teoria/06.%20Textures/Tangent%20Space%20Calculation.pdf

#ifdef SKIN
const uint DST_STRIDE = 20;
const uint SRC_STRIDE = 13;
#else
const uint DST_STRIDE = 12;
const uint SRC_STRIDE = 5;
#endif

Vertex read_vertex(uint idx) {
    idx *= SRC_STRIDE;
    idx += push_constants.base_vertex;

    float x = src_buf[idx];
    float y = src_buf[++idx];
    float z = src_buf[++idx];
    float u = src_buf[++idx];
    float v = src_buf[++idx];
    vec3 position = vec3(x, y, z);
    vec2 texcoord = vec2(u, v);

#ifndef SKIN
    return Vertex(position, texcoord);
#else
    float j0 = src_buf[++idx];
    float j1 = src_buf[++idx];
    float j2 = src_buf[++idx];
    float j3 = src_buf[++idx];
    float w0 = src_buf[++idx];
    float w1 = src_buf[++idx];
    float w2 = src_buf[++idx];
    float w3 = src_buf[++idx];
    vec4 joints = vec4(j0, j1, j2, j3);
    vec4 weights = vec4(w0, w1, w2, w3);

    return Vertex(position, texcoord, joints, weights);
#endif
}

void write_vertex(Vertex vertex, vec3 normal, vec4 tangent, uint idx) {
    idx *= DST_STRIDE;
    idx += push_constants.base_vertex;

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

#ifdef SKIN
    dst_buf[++idx] = vertex.joints.x;
    dst_buf[++idx] = vertex.joints.y;
    dst_buf[++idx] = vertex.joints.z;
    dst_buf[++idx] = vertex.joints.w;
    dst_buf[++idx] = vertex.weights.x;
    dst_buf[++idx] = vertex.weights.y;
    dst_buf[++idx] = vertex.weights.z;
    dst_buf[++idx] = vertex.weights.w;
#endif

    dst_buf[++idx] = vertex.texcoord.x;
    dst_buf[++idx] = vertex.texcoord.y;
}

void calc_vertex_attrs() {
    uint idx_a = 3 * gl_GlobalInvocationID.x + push_constants.base_idx;
    uint idx_b = idx_a + 1;
    uint idx_c = idx_a + 2;
    uint a_idx = read_idx(idx_a);
    uint b_idx = read_idx(idx_b);
    uint c_idx = read_idx(idx_c);

    Vertex a = read_vertex(a_idx);
    Vertex b = read_vertex(b_idx);
    Vertex c = read_vertex(c_idx);

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

    // The write mask tells us if we are allowed to write these vertices
    uint a_mask = 1 & write_mask[idx_a >> 5] >> idx_a % 32;
    uint b_mask = 1 & write_mask[idx_b >> 5] >> idx_b % 32;
    uint c_mask = 1 & write_mask[idx_c >> 5] >> idx_c % 32;

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
