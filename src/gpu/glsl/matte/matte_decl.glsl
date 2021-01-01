layout(location = 0) in vec2 image_uv;
layout(location = 1) in vec2 matte_uv;
layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform sampler2D image_sampler;
layout(set = 0, binding = 1) uniform sampler2D matte_sampler;
