#include "calc_vertex_attrs.glsl"

// Reads a 32 bit index out of the 32 bit index buffer
uint read_idx(uint idx) {
    return idx_buf[idx];
}

#include "calc_vertex_attrs_fn.glsl"

void main() {
    calc_vertex_attrs();
}
