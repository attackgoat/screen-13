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

pub struct DescriptorPool {
    driver: Driver,
    max_sets: usize,
    ptr: Option<<_Backend as Backend>::DescriptorPool>,
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
        let desc_pool = {
            let device = driver.as_ref().borrow();

            unsafe { device.create_descriptor_pool(max_sets, desc_ranges, flags) }.unwrap()
        };

        Self {
            driver,
            max_sets,
            ptr: Some(desc_pool),
        }
    }

    pub fn max_sets(pool: &Self) -> usize {
        pool.max_sets
    }
}

impl AsMut<<_Backend as Backend>::DescriptorPool> for DescriptorPool {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::DescriptorPool {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::DescriptorPool> for DescriptorPool {
    fn as_ref(&self) -> &<_Backend as Backend>::DescriptorPool {
        &*self
    }
}

impl Deref for DescriptorPool {
    type Target = <_Backend as Backend>::DescriptorPool;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for DescriptorPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        let device = self.driver.as_ref().borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_descriptor_pool(ptr);
        }
    }
}
