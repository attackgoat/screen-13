use {super::{*, Backend}, gfx_hal::pso::AllocationError};

#[derive(Debug)]
pub struct DescriptorPoolMock;

impl DescriptorPool<Backend> for DescriptorPoolMock {
    unsafe fn allocate_set(
        &mut self,
        _layout: &DescriptorSetLayoutMock,
    ) -> Result<DescriptorSetMock, AllocationError> {
        Ok(DescriptorSetMock {
            name: String::new(),
        })
    }

    unsafe fn free<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSetMock>,
    {
    }

    unsafe fn reset(&mut self) {}
}
