#version 450

const float U[6] = {0, 0, 1, 1, 1, 0};
const float V[6] = {0, 1, 0, 1, 0, 1};
const float X[6] = {-1, -1, 1, 1, 1, -1};
const float Y[6] = {-1, 1, -1, 1, -1, 1};

layout(location = 0) out vec2 texcoord_out;

// Returns the quad billboard coordinate for the current vertex. This quad is placed at (-1,-1) and
// extends to (1,1). Draw mode is CCW front faces. Obviously draw six vertices only.
vec2 vertex_pos() {
    float x = X[gl_VertexIndex];
    float y = Y[gl_VertexIndex];

    return vec2(x, y);
}

// Returns the quad billboard coordinate for the current vertex. Texture coordinates start at (0,0)
// and evenly texture to (1,1). Obviously draw six vertices only.
vec2 vertex_tex() {
    float u = U[gl_VertexIndex];
    float v = V[gl_VertexIndex];

    return vec2(u, v);
}

void main() {
    texcoord_out = vertex_pos(); // TODO: Untangle this back to vertex_tex
    gl_Position = vec4(vertex_pos(), 0, 1);
}
