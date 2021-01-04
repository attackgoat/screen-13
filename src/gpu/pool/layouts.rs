use {
    crate::gpu::{
        def::{desc_set_layout, push_const},
        driver::{DescriptorSetLayout, PipelineLayout},
        Driver,
    },
    gfx_hal::pso::{DescriptorSetLayoutBinding, ShaderStageFlags},
    std::{borrow::Borrow, iter::once, ops::Range},
};

#[derive(Default)]
pub struct Layouts {
    compute_calc_vertex_attrs: Option<(DescriptorSetLayout, PipelineLayout)>,
    compute_decode_rgb_rgba: Option<(DescriptorSetLayout, PipelineLayout)>,
}

impl Layouts {
    fn lazy_init<I, P>(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        layouts: &mut Option<(DescriptorSetLayout, PipelineLayout)>,
        bindings: I,
        push_consts: P,
    ) where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayoutBinding>,
        P: IntoIterator,
        P::Item: Borrow<(ShaderStageFlags, Range<u32>)>,
        P::IntoIter: ExactSizeIterator,
    {
        if layouts.is_none() {
            let desc_set_layout = DescriptorSetLayout::new(
                #[cfg(feature = "debug-names")]
                name,
                driver,
                bindings,
            );
            let pipeline_layout = PipelineLayout::new(
                #[cfg(feature = "debug-names")]
                name,
                driver,
                once(desc_set_layout.as_ref()),
                push_consts,
            );
            *layouts = Some((desc_set_layout, pipeline_layout));
        }
    }

    pub(crate) fn compute_calc_vertex_attrs(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
    ) -> &(DescriptorSetLayout, PipelineLayout) {
        Self::lazy_init(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            &mut self.compute_calc_vertex_attrs,
            &desc_set_layout::CALC_VERTEX_ATTRS,
            &push_const::CALC_VERTEX_ATTRS,
        );

        self.compute_calc_vertex_attrs.as_ref().unwrap()
    }

    pub(crate) fn compute_decode_rgb_rgba(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
    ) -> &(DescriptorSetLayout, PipelineLayout) {
        Self::lazy_init(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            &mut self.compute_decode_rgb_rgba,
            &desc_set_layout::DECODE_RGB_RGBA,
            &push_const::DECODE_RGB_RGBA,
        );

        self.compute_decode_rgb_rgba.as_ref().unwrap()
    }
}
