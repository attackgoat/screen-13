struct VertexIn {
    uint material;
    vec3 position;
    vec2 texcoord;

#ifdef SKIN
    vec4 joints;
    vec4 weights;
#endif
};

struct VertexOut {
    uint material;
    vec3 normal;
    vec3 position;
    vec4 tangent;
    vec2 texcoord;

#ifdef SKIN
    vec4 joints;
    vec4 weights;
#endif
};

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0, std430) readonly buffer IndexBuffer {
    uint idx_buf[];
};

layout(set = 0, binding = 1, std430) readonly buffer SourceBuffer {
    VertexIn src_buf[];
};

layout(set = 0, binding = 2, std430) writeonly buffer DestinationBuffer {
    VertexOut dst_buf[];
};


