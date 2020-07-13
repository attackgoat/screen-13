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
    ptr: Option<<_Backend as Backend>::PipelineLayout>,
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
            driver,
            ptr: Some(pipeline_layout),
        }
    }
}

impl AsMut<<_Backend as Backend>::PipelineLayout> for PipelineLayout {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::PipelineLayout {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::PipelineLayout> for PipelineLayout {
    fn as_ref(&self) -> &<_Backend as Backend>::PipelineLayout {
        &*self
    }
}

impl Deref for PipelineLayout {
    type Target = <_Backend as Backend>::PipelineLayout;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for PipelineLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_pipeline_layout(ptr);
        }
    }
}
