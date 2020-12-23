use {
    super::spirv,
    crate::{pak::IndexType,gpu::driver::{
        descriptor_range_desc, descriptor_set_layout_binding, ComputePipeline, DescriptorPool,
        DescriptorSetLayout, Driver, Sampler, ShaderModule,
    }},
    gfx_hal::{
        pso::{
            BufferDescriptorFormat, BufferDescriptorType, DescriptorPool as _, DescriptorRangeDesc,
            DescriptorSetLayoutBinding, DescriptorType, ImageDescriptorType, ShaderStageFlags,
        },
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        iter::{empty, once},
        ops::Range,
    },
};

pub struct Compute {
    desc_pool: DescriptorPool,
    desc_sets: Vec<<_Backend as Backend>::DescriptorSet>,
    max_desc_sets: usize,
    pipeline: ComputePipeline,
    set_layout: DescriptorSetLayout,
    samplers: Vec<Sampler>,
    shader: ShaderModule,
}

impl Compute {
    #[allow(clippy::too_many_arguments)]
    fn new<I, IR, ID, IS>(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        spirv: &[u32],
        consts: IR,
        max_desc_sets: usize,
        desc_ranges: ID,
        bindings: I,
        samplers: IS,
    ) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
        IR: IntoIterator,
        IR::IntoIter: ExactSizeIterator,
        IR::Item: Borrow<(ShaderStageFlags, Range<u32>)>,
        ID: IntoIterator,
        ID::IntoIter: ExactSizeIterator,
        ID::Item: Borrow<DescriptorRangeDesc>,
        IS: Iterator<Item = Sampler>,
    {
        let shader = unsafe { ShaderModule::new(Driver::clone(&driver), spirv) };
        let set_layout = DescriptorSetLayout::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            bindings,
        );
        let pipeline = unsafe {
            ComputePipeline::new(
                #[cfg(debug_assertions)]
                name,
                Driver::clone(&driver),
                ShaderModule::entry_point(&shader),
                once(&*set_layout),
                consts,
            )
        };
        let mut desc_pool = DescriptorPool::new(Driver::clone(&driver), max_desc_sets, desc_ranges);
        let layouts = (0..max_desc_sets).map(|_| &*set_layout);
        let mut desc_sets = Vec::with_capacity(max_desc_sets);

        unsafe {
            desc_pool.allocate(layouts, &mut desc_sets).unwrap();
        }

        let samplers = samplers.collect();

        Compute {
            desc_pool,
            desc_sets,
            max_desc_sets,
            pipeline,
            set_layout,
            samplers,
            shader,
        }
    }

    fn calc_vertex_attrs(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        spirv: &[u32],
        idx_ty: IndexType,
        max_desc_sets: usize,
        skin: bool,
    ) -> Self {
        Self::new(
            #[cfg(debug_assertions)]
            name,
            driver,
            spirv,
            &[(ShaderStageFlags::COMPUTE, 0..4)],
            max_desc_sets,
            &[
                descriptor_range_desc(
                    3 * max_desc_sets,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_range_desc(
                    max_desc_sets,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: false },
                    },
                ),
            ],
            &[
                descriptor_set_layout_binding(
                    0,
                    1,
                    ShaderStageFlags::COMPUTE,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_set_layout_binding(
                    1,
                    1,
                    ShaderStageFlags::COMPUTE,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_set_layout_binding(
                    2,
                    1,
                    ShaderStageFlags::COMPUTE,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: false },
                    },
                ),
                descriptor_set_layout_binding(
                    3,
                    1,
                    ShaderStageFlags::COMPUTE,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
            ],
            empty(),
        )
    }

    pub fn calc_vertex_attrs_u16(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(debug_assertions)]
            name,
            driver,
            &spirv::compute::CALC_VERTEX_ATTRS_U16_COMP,
            IndexType::U16,
            max_desc_sets,
            false,
        )
    }

    pub fn calc_vertex_attrs_u16_skin(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(debug_assertions)]
            name,
            driver,
            &spirv::compute::CALC_VERTEX_ATTRS_U16_SKIN_COMP,
            IndexType::U16,
            max_desc_sets,
            true,
        )
    }

    pub fn calc_vertex_attrs_u32(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(debug_assertions)]
            name,
            driver,
            &spirv::compute::CALC_VERTEX_ATTRS_U32_COMP,
            IndexType::U32,
            max_desc_sets,
            false,
        )
    }

    pub fn calc_vertex_attrs_u32_skin(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        max_desc_sets: usize,
    ) -> Self {
        Self::calc_vertex_attrs(
            #[cfg(debug_assertions)]
            name,
            driver,
            &spirv::compute::CALC_VERTEX_ATTRS_U32_SKIN_COMP,
            IndexType::U32,
            max_desc_sets,
            true,
        )
    }

    pub fn decode_rgb_rgba(
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        max_desc_sets: usize,
    ) -> Self {
        Self::new(
            #[cfg(debug_assertions)]
            name,
            driver,
            &spirv::compute::DECODE_RGB_RGBA_COMP,
            &[(ShaderStageFlags::COMPUTE, 0..4)],
            max_desc_sets,
            &[
                descriptor_range_desc(
                    1,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_range_desc(
                    1,
                    DescriptorType::Image {
                        ty: ImageDescriptorType::Storage { read_only: false },
                    },
                ),
            ],
            &[
                descriptor_set_layout_binding(
                    0,
                    1,
                    ShaderStageFlags::COMPUTE,
                    DescriptorType::Buffer {
                        format: BufferDescriptorFormat::Structured {
                            dynamic_offset: false,
                        },
                        ty: BufferDescriptorType::Storage { read_only: true },
                    },
                ),
                descriptor_set_layout_binding(
                    1,
                    1,
                    ShaderStageFlags::COMPUTE,
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

    fn reset(&mut self) {
        unsafe {
            self.desc_pool.reset();
        }

        for desc_set in &mut self.desc_sets {
            *desc_set = unsafe { self.desc_pool.allocate_set(&*self.set_layout).unwrap() }
        }
    }

    pub fn desc_set(&self, idx: usize) -> &<_Backend as Backend>::DescriptorSet {
        &self.desc_sets[idx]
    }
}
