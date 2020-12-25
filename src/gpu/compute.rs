use {
    super::spirv,
    crate::gpu::driver::{
        descriptor_range_desc, descriptor_set_layout_binding, ComputePipeline, DescriptorPool,
        DescriptorSetLayout, Driver, PipelineLayout, Sampler, ShaderModule,
    },
    gfx_hal::{
        pso::{
            BufferDescriptorFormat, BufferDescriptorType, DescriptorPool as _, DescriptorRangeDesc,
            DescriptorSetLayoutBinding, DescriptorType, ImageDescriptorType, ShaderStageFlags,
        },
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{borrow::Borrow, iter::empty, ops::Range},
};

pub struct Compute {
    desc_pool: DescriptorPool,
    desc_sets: Vec<<_Backend as Backend>::DescriptorSet>,
    max_desc_sets: usize,
    pipeline: ComputePipeline,
    samplers: Vec<Sampler>,
    shader: ShaderModule,
}

impl Compute {
    pub const CALC_VERTEX_ATTRS_DESC_SET_LAYOUT: [DescriptorSetLayoutBinding; 4] = [
        descriptor_set_layout_binding(
            0, // idx_buf
            ShaderStageFlags::COMPUTE,
            DescriptorType::Buffer {
                format: Compute::STRUCTURED_BUF,
                ty: Compute::READ_ONLY_BUF,
            },
        ),
        descriptor_set_layout_binding(
            1, // src_buf
            ShaderStageFlags::COMPUTE,
            DescriptorType::Buffer {
                format: Compute::STRUCTURED_BUF,
                ty: Compute::READ_ONLY_BUF,
            },
        ),
        descriptor_set_layout_binding(
            2, // dst_buf
            ShaderStageFlags::COMPUTE,
            DescriptorType::Buffer {
                format: Compute::STRUCTURED_BUF,
                ty: Compute::READ_WRITE_BUF,
            },
        ),
        descriptor_set_layout_binding(
            3, // write_mask
            ShaderStageFlags::COMPUTE,
            DescriptorType::Buffer {
                format: Compute::STRUCTURED_BUF,
                ty: Compute::READ_ONLY_BUF,
            },
        ),
    ];
    pub const CALC_VERTEX_ATTRS_PUSH_CONSTS: [(ShaderStageFlags, Range<u32>); 1] =
        [(ShaderStageFlags::COMPUTE, 0..8)];
    pub const DECODE_RGB_RGBA_DESC_SET_LAYOUT: [DescriptorSetLayoutBinding; 2] = [
        descriptor_set_layout_binding(
            0, // pixel_buf
            ShaderStageFlags::COMPUTE,
            DescriptorType::Buffer {
                format: Compute::STRUCTURED_BUF,
                ty: Compute::READ_ONLY_BUF,
            },
        ),
        descriptor_set_layout_binding(
            1, // image
            ShaderStageFlags::COMPUTE,
            DescriptorType::Image {
                ty: Compute::READ_WRITE_STORAGE_IMAGE,
            },
        ),
    ];
    pub const DECODE_RGB_RGBA_PUSH_CONSTS: [(ShaderStageFlags, Range<u32>); 1] =
        [(ShaderStageFlags::COMPUTE, 0..4)];
    const READ_ONLY_BUF: BufferDescriptorType = BufferDescriptorType::Storage { read_only: true };
    const READ_WRITE_BUF: BufferDescriptorType = BufferDescriptorType::Storage { read_only: false };
    const READ_WRITE_STORAGE_IMAGE: ImageDescriptorType =
        ImageDescriptorType::Storage { read_only: false };
    const STRUCTURED_BUF: BufferDescriptorFormat = BufferDescriptorFormat::Structured {
        dynamic_offset: false,
    };

    #[allow(clippy::too_many_arguments)]
    unsafe fn new<ID, IS>(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
        spirv: &[u32],
        desc_ranges: ID,
        samplers: IS,
    ) -> Self
    where
        ID: IntoIterator,
        ID::IntoIter: ExactSizeIterator,
        ID::Item: Borrow<DescriptorRangeDesc>,
        IS: Iterator<Item = Sampler>,
    {
        let shader = ShaderModule::new(driver, spirv);
        let pipeline = ComputePipeline::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            pipeline_layout.as_ref(),
            ShaderModule::entry_point(&shader),
        );
        let mut desc_pool = DescriptorPool::new(driver, max_desc_sets, desc_ranges);
        let layouts = (0..max_desc_sets).map(|_| desc_set_layout.as_ref());
        let mut desc_sets = Vec::with_capacity(max_desc_sets);

        desc_pool.allocate(layouts, &mut desc_sets).unwrap();

        let samplers = samplers.collect();

        Compute {
            desc_pool,
            desc_sets,
            max_desc_sets,
            pipeline,
            samplers,
            shader,
        }
    }

    unsafe fn calc_vertex_attrs(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
        spirv: &[u32],
    ) -> Self {
        Self::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            spirv,
            &[
                descriptor_range_desc(
                    3 * max_desc_sets,
                    DescriptorType::Buffer {
                        format: Compute::STRUCTURED_BUF,
                        ty: Compute::READ_ONLY_BUF,
                    },
                ),
                descriptor_range_desc(
                    max_desc_sets,
                    DescriptorType::Buffer {
                        format: Compute::STRUCTURED_BUF,
                        ty: Compute::READ_WRITE_BUF,
                    },
                ),
            ],
            empty(),
        )
    }

    /// Safety: Don't let desc_set_layout or pipeline_layout drop before this!
    pub unsafe fn calc_vertex_attrs_u16(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            &spirv::compute::CALC_VERTEX_ATTRS_U16_COMP,
        )
    }

    /// Safety: Don't let desc_set_layout or pipeline_layout drop before this!
    pub unsafe fn calc_vertex_attrs_u16_skin(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            &spirv::compute::CALC_VERTEX_ATTRS_U16_SKIN_COMP,
        )
    }

    /// Safety: Don't let desc_set_layout or pipeline_layout drop before this!
    pub unsafe fn calc_vertex_attrs_u32(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            &spirv::compute::CALC_VERTEX_ATTRS_U32_COMP,
        )
    }

    /// Safety: Don't let desc_set_layout or pipeline_layout drop before this!
    pub unsafe fn calc_vertex_attrs_u32_skin(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            &spirv::compute::CALC_VERTEX_ATTRS_U32_SKIN_COMP,
        )
    }

    pub unsafe fn decode_rgb_rgba(
        #[cfg(feature = "debug-names")] name: &str,
        driver: &Driver,
        desc_set_layout: &DescriptorSetLayout,
        pipeline_layout: &PipelineLayout,
        max_desc_sets: usize,
    ) -> Self {
        Self::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            &spirv::compute::DECODE_RGB_RGBA_COMP,
            &[
                descriptor_range_desc(
                    max_desc_sets,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_range_desc(
                    max_desc_sets,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Storage { read_only: false },
                    },
                ),
            ],
            empty(),
        )
    }

    pub fn max_desc_sets(&self) -> usize {
        self.max_desc_sets
    }

    pub fn pipeline(&self) -> &ComputePipeline {
        &self.pipeline
    }

    pub fn desc_set(&self, idx: usize) -> &<_Backend as Backend>::DescriptorSet {
        &self.desc_sets[idx]
    }
}
