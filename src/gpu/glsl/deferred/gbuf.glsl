#version 450

layout(location = 0) in vec2 texcoord;

layout(input_attachment_index = 0,
       binding = 0) uniform subpassInput material_sampler;
layout(input_attachment_index = 1,
       binding = 1) uniform subpassInput normal_sampler;
layout(input_attachment_index = 2,
       binding = 2) uniform subpassInput position_depth_sampler;

layout(location = 0) out vec4 color;
