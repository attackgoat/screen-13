use {
    gfx_hal::{
        command::{BufferCopy, CommandBuffer as _},
        format::{ChannelType, Format, SurfaceType},
        pso::{
            DescriptorArrayIndex, DescriptorBinding, DescriptorRangeDesc,
            DescriptorSetLayoutBinding, DescriptorType, ShaderStageFlags,
        },
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::iter::{empty, once},
};

pub unsafe fn bind_compute_descriptor_set(
    cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
    layout: &<_Backend as Backend>::PipelineLayout,
    desc_set: &<_Backend as Backend>::DescriptorSet,
) {
    cmd_buf.bind_compute_descriptor_sets(layout, 0, once(desc_set), empty());
}

pub unsafe fn bind_graphics_descriptor_set(
    cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
    layout: &<_Backend as Backend>::PipelineLayout,
    desc_set: &<_Backend as Backend>::DescriptorSet,
) {
    cmd_buf.bind_graphics_descriptor_sets(layout, 0, once(desc_set), empty());
}

pub const fn buffer_copy(len: u64) -> BufferCopy {
    BufferCopy {
        dst: 0,
        size: len,
        src: 0,
    }
}

pub fn change_channel_type(format: Format, ty: ChannelType) -> Format {
    match format.base_format().0 {
        SurfaceType::R8_G8_B8_A8 => match ty {
            ChannelType::Uint => Format::Rgba8Uint,
            ChannelType::Unorm => Format::Rgba8Unorm,
            ChannelType::Srgb => Format::Rgba8Srgb,
            _ => panic!(),
        },
        SurfaceType::B8_G8_R8_A8 => match ty {
            ChannelType::Uint => Format::Bgra8Uint,
            ChannelType::Unorm => Format::Bgra8Unorm,
            ChannelType::Srgb => Format::Bgra8Srgb,
            _ => panic!(),
        },
        SurfaceType::A8_B8_G8_R8 => match ty {
            ChannelType::Uint => Format::Abgr8Uint,
            ChannelType::Unorm => Format::Abgr8Unorm,
            ChannelType::Srgb => Format::Abgr8Srgb,
            _ => panic!(),
        },
        _ => panic!(),
    }
}

pub const fn descriptor_range_desc(count: usize, ty: DescriptorType) -> DescriptorRangeDesc {
    DescriptorRangeDesc { ty, count }
}

pub const fn descriptor_set_layout_binding(
    binding: DescriptorBinding,
    stage_flags: ShaderStageFlags,
    ty: DescriptorType,
) -> DescriptorSetLayoutBinding {
    descriptor_set_layout_binding_count(binding, 1, stage_flags, ty)
}

pub const fn descriptor_set_layout_binding_count(
    binding: DescriptorBinding,
    count: DescriptorArrayIndex,
    stage_flags: ShaderStageFlags,
    ty: DescriptorType,
) -> DescriptorSetLayoutBinding {
    DescriptorSetLayoutBinding {
        binding,
        count,
        immutable_samplers: false,
        stage_flags,
        ty,
    }
}
