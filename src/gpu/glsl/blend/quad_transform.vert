#include "../quad_transform.glsl"

layout(location = 1) out vec2 texcoord_base_out;

void main() {
    texcoord_base_out = vertex();
    texcoord_out = texcoord_base_out * push_constants.texcoord_scale + push_constants.texcoord_offset;
    gl_Position = push_constants.vertex_transform * vec4(texcoord_base_out, 0, 1);
}
