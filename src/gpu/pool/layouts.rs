use {
    crate::gpu::{
        def::{desc_set_layout, push_const},
        driver::{DescriptorSetLayout, PipelineLayout},
    },
    gfx_hal::pso::{DescriptorSetLayoutBinding, ShaderStageFlags},
    std::{iter::once, ops::Range},
};

#[derive(Default)]
pub struct Layouts {
    compute_calc_vertex_attrs: Option<(DescriptorSetLayout, PipelineLayout)>,
    compute_decode_rgb_rgba: Option<(DescriptorSetLayout, PipelineLayout)>,
}

impl Layouts {
    unsafe fn lazy_init<Ib, Ip>(
        #[cfg(feature = "debug-names")] name: &str,
        layouts: &mut Option<(DescriptorSetLayout, PipelineLayout)>,
        bindings: Ib,
        push_consts: Ip,
    ) where
        Ib: Iterator<Item = DescriptorSetLayoutBinding>,
        Ip: Iterator<Item = (ShaderStageFlags, Range<u32>)>,
    {
        if layouts.is_none() {
            let desc_set_layout = DescriptorSetLayout::new(
                #[cfg(feature = "debug-names")]
                name,
                bindings,
            );
            let pipeline_layout = PipelineLayout::new(
                #[cfg(feature = "debug-names")]
                name,
                once(desc_set_layout.as_ref()),
                push_consts,
            );
            *layouts = Some((desc_set_layout, pipeline_layout));
        }
    }

    pub(crate) unsafe fn compute_calc_vertex_attrs(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> &(DescriptorSetLayout, PipelineLayout) {
        Self::lazy_init(
            #[cfg(feature = "debug-names")]
            name,
            &mut self.compute_calc_vertex_attrs,
            desc_set_layout::CALC_VERTEX_ATTRS.to_vec().drain(..),
            push_const::CALC_VERTEX_ATTRS.to_vec().drain(..),
        );

        self.compute_calc_vertex_attrs.as_ref().unwrap()
    }

    pub(crate) unsafe fn compute_decode_rgb_rgba(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> &(DescriptorSetLayout, PipelineLayout) {
        Self::lazy_init(
            #[cfg(feature = "debug-names")]
            name,
            &mut self.compute_decode_rgb_rgba,
            desc_set_layout::DECODE_RGB_RGBA.to_vec().drain(..),
            push_const::DECODE_RGB_RGBA.to_vec().drain(..),
        );

        self.compute_decode_rgb_rgba.as_ref().unwrap()
    }
}
