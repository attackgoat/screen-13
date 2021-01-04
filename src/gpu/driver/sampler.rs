use {
    super::Device,
    gfx_hal::{
        device::Device as _,
        image::{Filter, Lod, PackedColor, SamplerDesc, WrapMode},
        pso::Comparison,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut, Range},
};

pub type SamplerBuilder = SamplerDesc;

pub struct Sampler {
    device: Device,
    ptr: Option<<_Backend as Backend>::Sampler>,
}

impl Sampler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: Device,
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
        let sampler = 
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
        .unwrap();

        Self {
            device,
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
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_sampler(ptr);
        }
    }
}
