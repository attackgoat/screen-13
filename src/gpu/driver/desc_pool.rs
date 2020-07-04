use {
    super::Driver,
    gfx_hal::{
        device::Device,
        pso::{DescriptorPoolCreateFlags, DescriptorRangeDesc},
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        ops::{Deref, DerefMut},
    },
};

#[derive(Debug)]
pub struct DescriptorPool {
    desc_pool: Option<<_Backend as Backend>::DescriptorPool>,
    driver: Driver,
    max_sets: usize,
}

impl DescriptorPool {
    pub fn new<I>(driver: Driver, max_sets: usize, desc_ranges: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorRangeDesc>,
    {
        Self::with_flags(
            driver,
            max_sets,
            desc_ranges,
            DescriptorPoolCreateFlags::empty(),
        )
    }

    pub fn with_flags<I>(
        driver: Driver,
        max_sets: usize,
        desc_ranges: I,
        flags: DescriptorPoolCreateFlags,
    ) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorRangeDesc>,
    {
        let desc_pool = unsafe {
            driver
                .as_ref()
                .borrow()
                .create_descriptor_pool(max_sets, desc_ranges, flags)
        }
        .unwrap();

        Self {
            desc_pool: Some(desc_pool),
            driver,
            max_sets,
        }
    }

    pub fn max_sets(&self) -> usize {
        self.max_sets
    }
}

impl AsRef<<_Backend as Backend>::DescriptorPool> for DescriptorPool {
    fn as_ref(&self) -> &<_Backend as Backend>::DescriptorPool {
        self.desc_pool.as_ref().unwrap()
    }
}

impl Deref for DescriptorPool {
    type Target = <_Backend as Backend>::DescriptorPool;

    fn deref(&self) -> &Self::Target {
        self.desc_pool.as_ref().unwrap()
    }
}

impl DerefMut for DescriptorPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.desc_pool.as_mut().unwrap()
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .as_ref()
                .borrow()
                .destroy_descriptor_pool(self.desc_pool.take().unwrap());
        }
    }
}
