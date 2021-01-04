use {
    super::{READ_ONLY_BUF, READ_WRITE_BUF, READ_WRITE_IMG},
    crate::gpu::{
        driver::{
            descriptor_range_desc, ComputePipeline, DescriptorPool, DescriptorSetLayout, Driver,
            PipelineLayout, Sampler, ShaderModule,
        },
        spirv,
    },
    gfx_hal::{
        pso::{DescriptorPool as _, DescriptorRangeDesc},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{borrow::Borrow, iter::empty},
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
        idx: bool,
    ) -> Self {
        let read_only_buf_count = if idx { 3 } else { 1 };

        Self::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            desc_set_layout,
            pipeline_layout,
            max_desc_sets,
            spirv,
            &[
                descriptor_range_desc(read_only_buf_count * max_desc_sets, READ_ONLY_BUF),
                descriptor_range_desc(max_desc_sets, READ_WRITE_BUF),
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
            &spirv::compute::calc_vertex_attrs_u16_comp::MAIN,
            true,
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
            &spirv::compute::calc_vertex_attrs_u16_comp::SKIN,
            true,
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
            &spirv::compute::calc_vertex_attrs_u32_comp::MAIN,
            true,
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
            &spirv::compute::calc_vertex_attrs_u32_comp::SKIN,
            true,
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
            &spirv::compute::decode_rgb_rgba_comp::MAIN,
            &[
                descriptor_range_desc(max_desc_sets, READ_ONLY_BUF),
                descriptor_range_desc(max_desc_sets, READ_WRITE_IMG),
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
