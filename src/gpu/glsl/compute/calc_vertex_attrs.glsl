layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(push_constant) uniform PushConstants {
    layout(offset = 0) uint offset;
} push_constants;

layout(set = 0, binding = 1, std430) buffer IdxBuffer {
    uint idx_buf[];
};

layout(set = 0, binding = 2, std430) buffer SrcBuffer {
    float src_buf[];
};

layout(set = 0, binding = 3, std430) buffer DstBuffer {
    float dst_buf[];
};

layout(set = 0, binding = 4, std430) buffer WriteMask {
    uint write_mask[];
};

struct Vertex {
    vec3 position;
    vec2 texcoord;

#ifdef SKIN
    vec4 joints;
    vec4 weights;
#endif
};
