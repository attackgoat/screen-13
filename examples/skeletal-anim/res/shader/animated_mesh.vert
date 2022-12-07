#version 460 core

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 world;
} push_const;

layout(binding = 0) uniform CameraUniform {
    mat4 projection;
    mat4 view;
    vec3 position;
} camera;

layout(std430, binding = 1) restrict readonly buffer AnimationBuffer {
    mat4[] joints;
} animation;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 texture;
layout(location = 3) in uint joint_indices;
layout(location = 4) in uint joint_weights;

layout(location = 0) out vec3 world_position_out;
layout(location = 1) out vec3 world_normal_out;
layout(location = 2) out vec2 texture_out;

void main() {
    mat4 joint = mat4(0.0);

    for (uint shift = 0; shift <= 24; shift += 8) {
        uint joint_index = (joint_indices >> shift) & 0xff;
        float joint_weight = float((joint_weights >> shift) & 0xff) / 255.0;

        if (joint_weight > 0.0) {
            joint += animation.joints[joint_index] * joint_weight;
        }
    }

    world_normal_out = normalize((
                            push_const.world
                            * transpose(inverse(joint))
                            * vec4(normal, 0.0)
                        ).xyz);
    world_position_out = (
                            push_const.world
                            * joint
                            * vec4(position, 1.0)
                        ).xyz;
    texture_out = texture;

    gl_Position = camera.projection
                * camera.view
                * vec4(world_position_out, 1.0);
}