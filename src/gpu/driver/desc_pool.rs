use {
    crate::gpu::device,
    gfx_hal::{
        device::Device as _,
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
    max_desc_sets: usize,
    ptr: Option<<_Backend as Backend>::DescriptorPool>,
}

impl DescriptorPool {
    pub unsafe fn new<I>(max_desc_sets: usize, desc_ranges: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorRangeDesc>,
        I::IntoIter: ExactSizeIterator,
    {
        Self::new_flags(
            max_desc_sets,
            desc_ranges,
            DescriptorPoolCreateFlags::empty(),
        )
    }

    pub unsafe fn new_flags<I>(
        max_desc_sets: usize,
        desc_ranges: I,
        flags: DescriptorPoolCreateFlags,
    ) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorRangeDesc>,
        I::IntoIter: ExactSizeIterator,
    {
        let ptr = device()
            .create_descriptor_pool(max_desc_sets, desc_ranges, flags)
            .unwrap();

        Self {
            max_desc_sets,
            ptr: Some(ptr),
        }
    }

    pub fn max_desc_sets(desc_pool: &Self) -> usize {
        desc_pool.max_desc_sets
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
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device().destroy_descriptor_pool(ptr);
        }
    }
}
