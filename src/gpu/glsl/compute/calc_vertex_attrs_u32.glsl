// NOTE: This program is intended to help "inflate" model vertex buffers which have been read from
// disk or the network. We do not store normal or tangent in the asset .pak file, so these
// attributes must be reconstructed at runtime; prior to rendering use.

#include "calc_vertex_attrs.glsl"

// Reads a 32 bit index out of the 32 bit index buffer
uint read_idx(uint idx) {
    return idx_buf[idx];
}

#include "calc_vertex_attrs_fn.glsl"

void main() {
    calc_vertex_attrs();
}
