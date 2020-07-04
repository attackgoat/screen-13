#version 450

const float X[4] = {
    0,
    0,
    0,
    0,
};
const float Y[4] = {
    0,
    0,
    0,
    0,
};

layout(location = 0) out vec2 texcoord;

void main() {
    texcoord = vec2(X[gl_VertexIndex], Y[gl_VertexIndex]);
    gl_Position = vec4(texcoord, 0, 1);
}
