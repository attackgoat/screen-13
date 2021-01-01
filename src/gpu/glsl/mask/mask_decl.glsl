layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform sampler2D global_mask_sampler;
layout(set = 0, binding = 1) uniform sampler2D local_mask_sampler;
