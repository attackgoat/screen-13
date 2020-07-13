#include "quad_transform.glsl"

void main() {
    texcoord_out = vertex() * push_constants.texcoord_scale + push_constants.texcoord_offset;
    gl_Position = push_constants.vertex_transform * vec4(vertex(), 0, 1);
}
