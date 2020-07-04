#include "../quad_transform.glsl"

layout(location = 1) out vec2 texcoord_base_out;

void main() {
    texcoord_base_out = get_texcoord();
    texcoord_out = get_texcoord();
    gl_Position = push_constants.transform * vec4(texcoord_out, 0, 1);
}
