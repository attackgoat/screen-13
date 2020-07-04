use {
    super::Driver,
    gfx_hal::{device::Device, pso::ShaderStageFlags, Backend},
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut, Range},
    },
};

#[derive(Debug)]
pub struct PipelineLayout {
    driver: Driver,
    pipeline_layout: Option<<_Backend as Backend>::PipelineLayout>,
}

impl PipelineLayout {
    pub fn new<IS, IR>(driver: Driver, set_layouts: IS, push_constant: IR) -> Self
    where
        IS: IntoIterator,
        IS::Item: Borrow<<_Backend as Backend>::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(ShaderStageFlags, Range<u32>)>,
    {
        let pipeline_layout = {
            let device = driver.as_ref().borrow();
            unsafe { device.create_pipeline_layout(set_layouts, push_constant) }.unwrap()
        };

        Self {
            pipeline_layout: Some(pipeline_layout),
            driver,
        }
    }
}

impl AsRef<<_Backend as Backend>::PipelineLayout> for PipelineLayout {
    fn as_ref(&self) -> &<_Backend as Backend>::PipelineLayout {
        self.pipeline_layout.as_ref().unwrap()
    }
}

impl Deref for PipelineLayout {
    type Target = <_Backend as Backend>::PipelineLayout;

    fn deref(&self) -> &Self::Target {
        self.pipeline_layout.as_ref().unwrap()
    }
}

impl DerefMut for PipelineLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.pipeline_layout.as_mut().unwrap()
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_pipeline_layout(self.pipeline_layout.take().unwrap());
        }
    }
}
