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

#[derive(Debug)]
pub struct Sampler {
    driver: Driver,
    sampler: Option<<_Backend as Backend>::Sampler>,
}

impl Sampler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        driver: Driver,
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
        let sampler = unsafe {
            driver.borrow().create_sampler(&SamplerDesc {
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
        .unwrap();

        Self {
            sampler: Some(sampler),
            driver,
        }
    }
}

impl AsRef<<_Backend as Backend>::Sampler> for Sampler {
    fn as_ref(&self) -> &<_Backend as Backend>::Sampler {
        self.sampler.as_ref().unwrap()
    }
}

impl Deref for Sampler {
    type Target = <_Backend as Backend>::Sampler;

    fn deref(&self) -> &Self::Target {
        self.sampler.as_ref().unwrap()
    }
}

impl DerefMut for Sampler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sampler.as_mut().unwrap()
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .destroy_sampler(self.sampler.take().unwrap());
        }
    }
}
