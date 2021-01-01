const float X[6] = {0, 0, 1, 1, 1, 0};
const float Y[6] = {0, 1, 0, 1, 0, 1};

layout(location = 0) out vec2 texcoord_out;

// Returns the quad billboard coordinate for the current vertex. This quad is placed at (0,0) and evenly textures to (1,1).
// Draw mode is CCW front faces. Obviously draw six vertices only.
vec2 vertex() {
    float x = X[gl_VertexIndex];
    float y = Y[gl_VertexIndex];

    return vec2(x, y);
}
