use {
    super::{Backend, *},
    gfx_hal::pso::AllocationError,
};

#[derive(Debug)]
pub struct DescriptorPoolMock;

impl DescriptorPool<Backend> for DescriptorPoolMock {
    unsafe fn allocate_one(&mut self, _layout: &()) -> Result<(), AllocationError> {
        Ok(())
    }

    unsafe fn free<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = ()>,
    {
    }

    unsafe fn reset(&mut self) {}
}
