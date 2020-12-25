use {
    super::Driver,
    gfx_hal::{
        device::Device,
        image::{Filter, Lod, PackedColor, SamplerDesc, WrapMode},
        pso::Comparison,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut, Range},
};

pub type SamplerBuilder = SamplerDesc;

pub struct Sampler {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::Sampler>,
}

impl Sampler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        driver: &Driver,
        min: Filter,
        mag: Filter,
        mip: Filter,
        wrap_mode: (WrapMode, WrapMode, WrapMode),
        lod: (Lod, Range<Lod>),
        comparison: Option<Comparison>,
        border: PackedColor,
        normalized: bool,
        anisotropy_clamp: Option<u8>,
    ) -> Self {
        let sampler = {
            let device = driver.borrow();
            unsafe {
                device.create_sampler(&SamplerDesc {
                    min_filter: min,
                    mag_filter: mag,
                    mip_filter: mip,
                    wrap_mode,
                    lod_bias: lod.0,
                    lod_range: lod.1,
                    comparison,
                    border,
                    normalized,
                    anisotropy_clamp,
                })
            }
        }
        .unwrap();

        Self {
            driver: Driver::clone(driver),
            ptr: Some(sampler),
        }
    }
}

impl AsMut<<_Backend as Backend>::Sampler> for Sampler {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Sampler {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Sampler> for Sampler {
    fn as_ref(&self) -> &<_Backend as Backend>::Sampler {
        &*self
    }
}

impl Deref for Sampler {
    type Target = <_Backend as Backend>::Sampler;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for Sampler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_sampler(ptr);
        }
    }
}
