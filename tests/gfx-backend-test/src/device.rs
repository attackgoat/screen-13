use {
    super::{Backend, *},
    gfx_hal::{
        buffer::{
            CreationError as BufferCreationError, Usage as BufferUsage,
            ViewCreationError as BufferViewCreationError,
        },
        device::CreationError as DeviceCreationError,
        image::{
            CreationError as ImageCreationError, Level as ImageLevel, Usage as ImageUsage,
            ViewCreationError as ImageViewCreationError,
        },
        memory::SparseFlags,
        pso::CreationError as PsoCreationError,
        query::CreationError as QueryCreationError,
    },
    std::{borrow::Borrow, ops::Range},
};

#[derive(Debug)]
pub struct DeviceMock;

impl Device<Backend> for DeviceMock {
    unsafe fn create_command_pool(
        &self,
        _: QueueFamilyId,
        _: CommandPoolCreateFlags,
    ) -> Result<CommandPoolMock, OutOfMemory> {
        Ok(CommandPoolMock)
    }

    unsafe fn destroy_command_pool(&self, _: CommandPoolMock) {}

    unsafe fn allocate_memory(
        &self,
        memory_type: MemoryTypeId,
        size: u64,
    ) -> Result<MemoryMock, AllocationError> {
        MemoryMock::allocate(memory_type, size)
    }

    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        _: IA,
        _: IS,
        _: ID,
    ) -> Result<(), OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<SubpassDesc<'a>>,
    {
        Ok(())
    }

    unsafe fn create_pipeline_layout<'a, Is, Ic>(
        &self,
        set_layouts: Is,
        push_constant: Ic,
    ) -> Result<<Backend as gfx_hal::Backend>::PipelineLayout, OutOfMemory>
    where
        Is: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::DescriptorSetLayout>,
        Is::IntoIter: ExactSizeIterator,
        Ic: IntoIterator<Item = (ShaderStageFlags, Range<u32>)>,
        Ic::IntoIter: ExactSizeIterator,
    {
        Ok(())
    }

    unsafe fn create_pipeline_cache(&self, _data: Option<&[u8]>) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn get_pipeline_cache_data(&self, _cache: &()) -> Result<Vec<u8>, OutOfMemory> {
        Ok(vec![0])
    }

    unsafe fn destroy_pipeline_cache(&self, _: ()) {}

    unsafe fn create_graphics_pipeline<'a>(
        &self,
        _: &GraphicsPipelineDesc<'a, Backend>,
        _: Option<&()>,
    ) -> Result<(), PsoCreationError> {
        Ok(())
    }

    unsafe fn create_compute_pipeline<'a>(
        &self,
        _: &ComputePipelineDesc<'a, Backend>,
        _: Option<&()>,
    ) -> Result<(), PsoCreationError> {
        Ok(())
    }

    unsafe fn merge_pipeline_caches<'a, I>(
        &self,
        _: &mut <Backend as gfx_hal::Backend>::PipelineCache,
        _: I,
    ) -> Result<(), OutOfMemory>
    where
        I: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::PipelineCache>,
        I::IntoIter: ExactSizeIterator,
    {
        Ok(())
    }

    unsafe fn create_framebuffer<I>(&self, _: &(), _: I, _: Extent) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn create_shader_module(&self, _: &[u32]) -> Result<(), ShaderError> {
        Ok(())
    }

    unsafe fn create_sampler(&self, _: &SamplerDesc) -> Result<(), AllocationError> {
        Ok(())
    }

    unsafe fn create_buffer(
        &self,
        size: u64,
        _: BufferUsage,
        _: SparseFlags,
    ) -> Result<BufferMock, CreationError> {
        Ok(BufferMock::new(size))
    }

    unsafe fn get_buffer_requirements(&self, buffer: &BufferMock) -> Requirements {
        Requirements {
            size: buffer.size,
            alignment: 1,
            type_mask: !0,
        }
    }

    unsafe fn bind_buffer_memory(
        &self,
        _memory: &MemoryMock,
        _: u64,
        _: &mut BufferMock,
    ) -> Result<(), BindError> {
        Ok(())
    }

    unsafe fn create_buffer_view(
        &self,
        _: &BufferMock,
        _: Option<Format>,
        _: SubRange,
    ) -> Result<(), ViewCreationError> {
        Ok(())
    }

    unsafe fn create_image(
        &self,
        kind: Kind,
        _: ImageLevel,
        _: Format,
        _: Tiling,
        _: ImageUsage,
        _: SparseFlags,
        _: ViewCapabilities,
    ) -> Result<ImageMock, ImageCreationError> {
        Ok(ImageMock::new(kind))
    }

    unsafe fn get_image_requirements(&self, image: &ImageMock) -> Requirements {
        image.requirements()
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        image: &ImageMock,
        subresource: Subresource,
    ) -> SubresourceFootprint {
        image.subresource_footprint(subresource)
    }

    unsafe fn bind_image_memory(
        &self,
        _memory: &MemoryMock,
        _: u64,
        _: &mut ImageMock,
    ) -> Result<(), BindError> {
        Ok(())
    }

    unsafe fn create_image_view(
        &self,
        _: &ImageMock,
        _: ViewKind,
        _: Format,
        _: Swizzle,
        _: ImageUsage,
        _: SubresourceRange,
    ) -> Result<(), ImageViewCreationError> {
        Ok(())
    }

    unsafe fn create_descriptor_pool<I>(
        &self,
        _: usize,
        _: I,
        _: DescriptorPoolCreateFlags,
    ) -> Result<DescriptorPoolMock, OutOfMemory> {
        Ok(DescriptorPoolMock)
    }

    unsafe fn create_descriptor_set_layout<'a, I, J>(
        &self,
        bindings: I,
        immutable_samplers: J,
    ) -> Result<<Backend as gfx_hal::Backend>::DescriptorSetLayout, OutOfMemory>
    where
        I: IntoIterator<Item = DescriptorSetLayoutBinding>,
        I::IntoIter: ExactSizeIterator,
        J: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::Sampler>,
        J::IntoIter: ExactSizeIterator,
    {
        Ok(())
    }

    unsafe fn write_descriptor_set<'a, I>(&self, _: DescriptorSetWrite<'a, Backend, I>)
    where
        I: IntoIterator<Item = Descriptor<'a, Backend>>,
        I::IntoIter: ExactSizeIterator,
    {
    }

    unsafe fn copy_descriptor_set<'a>(&self, _: DescriptorSetCopy<'a, Backend>) {}

    fn create_semaphore(&self) -> Result<(), OutOfMemory> {
        Ok(())
    }

    fn create_fence(&self, _: bool) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn get_fence_status(&self, _: &()) -> Result<bool, DeviceLost> {
        Ok(true)
    }

    fn create_event(&self) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn get_event_status(&self, _: &()) -> Result<bool, WaitError> {
        Ok(true)
    }

    unsafe fn set_event(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn reset_event(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn create_query_pool(&self, _: Type, _: u32) -> Result<(), QueryCreationError> {
        Ok(())
    }

    unsafe fn destroy_query_pool(&self, _: ()) {}

    unsafe fn get_query_pool_results(
        &self,
        _: &(),
        _: Range<Id>,
        _: &mut [u8],
        _: Stride,
        _: ResultFlags,
    ) -> Result<bool, WaitError> {
        Ok(true)
    }

    unsafe fn map_memory(
        &self,
        memory: &mut MemoryMock,
        segment: Segment,
    ) -> Result<*mut u8, MapError> {
        memory.map(segment)
    }

    unsafe fn unmap_memory(&self, _memory: &mut MemoryMock) {}

    unsafe fn flush_mapped_memory_ranges<'a, I>(&self, _: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a MemoryMock, Segment)>,
    {
        Ok(())
    }

    unsafe fn invalidate_mapped_memory_ranges<'a, I>(&self, _: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a MemoryMock, Segment)>,
    {
        Ok(())
    }

    unsafe fn free_memory(&self, _memory: MemoryMock) {}

    unsafe fn destroy_shader_module(&self, _: ()) {}

    unsafe fn destroy_render_pass(&self, _: ()) {}

    unsafe fn destroy_pipeline_layout(&self, _: ()) {}

    unsafe fn destroy_graphics_pipeline(&self, _: ()) {}

    unsafe fn destroy_compute_pipeline(&self, _: ()) {}

    unsafe fn destroy_framebuffer(&self, _: ()) {}

    unsafe fn destroy_buffer(&self, _: BufferMock) {}

    unsafe fn destroy_buffer_view(&self, _: ()) {}

    unsafe fn destroy_image(&self, _: ImageMock) {}

    unsafe fn destroy_image_view(&self, _: ()) {}

    unsafe fn destroy_sampler(&self, _: ()) {}

    unsafe fn destroy_descriptor_pool(&self, _: DescriptorPoolMock) {}

    unsafe fn destroy_descriptor_set_layout(&self, _: ()) {}

    unsafe fn destroy_fence(&self, _: ()) {}

    unsafe fn destroy_semaphore(&self, _: ()) {}

    unsafe fn destroy_event(&self, _: ()) {}

    fn wait_idle(&self) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn set_image_name(&self, _: &mut ImageMock, _: &str) {}

    unsafe fn set_buffer_name(&self, _: &mut BufferMock, _: &str) {}

    unsafe fn set_command_buffer_name(&self, _: &mut CommandBufferMock, _: &str) {}

    unsafe fn set_semaphore_name(&self, _: &mut (), _: &str) {}

    unsafe fn set_fence_name(&self, _: &mut (), _: &str) {}

    unsafe fn set_framebuffer_name(&self, _: &mut (), _: &str) {}

    unsafe fn set_render_pass_name(&self, _: &mut (), _: &str) {}

    unsafe fn set_descriptor_set_name(&self, set: &mut (), name: &str) {}

    unsafe fn set_descriptor_set_layout_name(&self, layout: &mut (), name: &str) {}

    unsafe fn set_pipeline_layout_name(&self, _pipeline_layout: &mut (), _name: &str) {
        todo!()
    }

    unsafe fn reset_fence(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn wait_for_fence(&self, _: &(), _: u64) -> Result<bool, WaitError> {
        Ok(true)
    }

    fn start_capture(&self) {}

    fn stop_capture(&self) {}
}
