#version 450

#include "quad_transform.glsl"

void main() {
    texcoord_out = vertex_tex() * push_constants.texcoord_scale + push_constants.texcoord_offset;
    gl_Position = push_constants.vertex_transform * vec4(vertex_tex(), 0, 1); // TODO: This should use vertex_pos() but I haven't modified the other matrices yet
}
