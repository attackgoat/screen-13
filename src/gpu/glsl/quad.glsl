#version 450

const float X[6] = {0, 1, 0, 1, 0, 1};
const float Y[6] = {0, 0, 1, 1, 1, 0};

layout(location = 0) out vec2 texcoord_out;

// Returns the quad billboard coordinate for the current vertex. This quad is placed at (0,0) and evenly textures to (1,1).
vec2 vertex() { return vec2(X[gl_VertexIndex], Y[gl_VertexIndex]); }
