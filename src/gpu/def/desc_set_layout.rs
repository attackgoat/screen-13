use {
    super::{READ_ONLY_BUF, READ_ONLY_IMG, READ_WRITE_BUF, READ_WRITE_IMG},
    crate::gpu::driver::descriptor_set_layout_binding,
    gfx_hal::pso::{DescriptorSetLayoutBinding, ShaderStageFlags},
};

// General-use layouts

pub const SINGLE_READ_ONLY_IMG: [DescriptorSetLayoutBinding; 1] = [descriptor_set_layout_binding(
    0,
    ShaderStageFlags::FRAGMENT,
    READ_ONLY_IMG,
)];

// Specific-use layouts

pub const BLEND: [DescriptorSetLayoutBinding; 2] = [
    descriptor_set_layout_binding(
        0, // blend
        ShaderStageFlags::FRAGMENT,
        READ_ONLY_IMG,
    ),
    descriptor_set_layout_binding(
        1, // base
        ShaderStageFlags::FRAGMENT,
        READ_ONLY_IMG,
    ),
];
pub const CALC_VERTEX_ATTRS: [DescriptorSetLayoutBinding; 4] = [
    descriptor_set_layout_binding(
        0, // idx_buf
        ShaderStageFlags::COMPUTE,
        READ_ONLY_BUF,
    ),
    descriptor_set_layout_binding(
        1, // src_buf
        ShaderStageFlags::COMPUTE,
        READ_ONLY_BUF,
    ),
    descriptor_set_layout_binding(
        2, // dst_buf
        ShaderStageFlags::COMPUTE,
        READ_WRITE_BUF,
    ),
    descriptor_set_layout_binding(
        3, // write_mask
        ShaderStageFlags::COMPUTE,
        READ_ONLY_BUF,
    ),
];
pub const DECODE_RGB_RGBA: [DescriptorSetLayoutBinding; 2] = [
    descriptor_set_layout_binding(
        0, // pixel_buf
        ShaderStageFlags::COMPUTE,
        READ_ONLY_BUF,
    ),
    descriptor_set_layout_binding(
        1, // image
        ShaderStageFlags::COMPUTE,
        READ_WRITE_IMG,
    ),
];
pub const DRAW_MESH: [DescriptorSetLayoutBinding; 3] = [
    descriptor_set_layout_binding(
        0, // color_sampler
        ShaderStageFlags::FRAGMENT,
        READ_ONLY_IMG,
    ),
    descriptor_set_layout_binding(
        1, // material_sampler
        ShaderStageFlags::FRAGMENT,
        READ_ONLY_IMG,
    ),
    descriptor_set_layout_binding(
        2, // normal_sampler
        ShaderStageFlags::FRAGMENT,
        READ_ONLY_IMG,
    ),
];
pub const SKYDOME: [DescriptorSetLayoutBinding; 6] = [
    descriptor_set_layout_binding(
        0, // cloud1_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
    descriptor_set_layout_binding(
        1, // cloud2_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
    descriptor_set_layout_binding(
        2, // moon_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
    descriptor_set_layout_binding(
        3, // sun_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
    descriptor_set_layout_binding(
        4, // tint1_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
    descriptor_set_layout_binding(
        5, // tint2_sampler
        ShaderStageFlags::FRAGMENT,
        READ_WRITE_IMG,
    ),
];
