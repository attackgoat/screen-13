#include "quad_transform.glsl"

void main() {
    texcoord_out = get_texcoord();
    gl_Position = push_constants.transform * vec4(texcoord_out, 0, 1);
}
