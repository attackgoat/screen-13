#include "calc_vertex_attrs.glsl"

// Reads a 16 bit index out of the 32 bit index buffer
uint read_idx(uint idx) {
    uint data = idx_buf[idx >> 1];
    uint lo = data & 0xffff;
    uint hi = data >> 16;
    uint odd = idx & 0x01;
    uint even = 1 - odd;

    return even * lo + odd * hi;
}

#include "calc_vertex_attrs_fn.glsl"

void main() {
    calc_vertex_attrs();
}
