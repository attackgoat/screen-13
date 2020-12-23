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
};
