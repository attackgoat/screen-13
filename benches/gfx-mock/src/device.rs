use {
    super::{*, Backend},
    gfx_hal::{
        buffer::{CreationError as BufferCreationError, ViewCreationError as BufferViewCreationError, Usage as BufferUsage},
        image::{CreationError as ImageCreationError, ViewCreationError as ImageViewCreationError, Usage as ImageUsage, Level as ImageLevel},
        pso::CreationError as PsoCreationError,
        device::CreationError as DeviceCreationError,
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

    unsafe fn create_pipeline_layout<IS, IR>(&self, _: IS, _: IR) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn create_pipeline_cache(&self, _data: Option<&[u8]>) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn get_pipeline_cache_data(&self, _cache: &()) -> Result<Vec<u8>, OutOfMemory> {
        todo!()
    }

    unsafe fn destroy_pipeline_cache(&self, _: ()) {
        todo!()
    }

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
        todo!()
    }

    unsafe fn merge_pipeline_caches<I>(&self, _: &(), _: I) -> Result<(), OutOfMemory> {
        todo!()
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

    unsafe fn create_buffer(&self, size: u64, _: BufferUsage) -> Result<BufferMock, CreationError> {
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
        todo!()
    }

    unsafe fn create_image(
        &self,
        kind: Kind,
        _: ImageLevel,
        _: Format,
        _: Tiling,
        _: ImageUsage,
        _: ViewCapabilities,
    ) -> Result<ImageMock, ImageCreationError> {
        Ok(ImageMock::new(kind))
    }

    unsafe fn get_image_requirements(&self, image: &ImageMock) -> Requirements {
        image.get_requirements()
    }

    unsafe fn get_image_subresource_footprint(
        &self,
        _: &ImageMock,
        _: Subresource,
    ) -> SubresourceFootprint {
        todo!()
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

    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        _bindings: I,
        _samplers: J,
    ) -> Result<DescriptorSetLayoutMock, OutOfMemory> {
        Ok(DescriptorSetLayoutMock {
            name: Default::default(),
        })
    }

    unsafe fn write_descriptor_set<'a, I>(&self, _: DescriptorSetWrite<'a, Backend, I>)
    where
        I: IntoIterator,
        I::Item: Borrow<Descriptor<'a, Backend>>,
    {
    }

    unsafe fn copy_descriptor_set<'a>(&self, _: DescriptorSetCopy<'a, Backend>) {
        todo!()
    }

    fn create_semaphore(&self) -> Result<(), OutOfMemory> {
        Ok(())
    }

    fn create_fence(&self, _: bool) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn get_fence_status(&self, _: &()) -> Result<bool, DeviceLost> {
        todo!()
    }

    fn create_event(&self) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn get_event_status(&self, _: &()) -> Result<bool, WaitError> {
        todo!()
    }

    unsafe fn set_event(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn reset_event(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        todo!()
    }

    unsafe fn create_query_pool(&self, _: Type, _: u32) -> Result<(), QueryCreationError> {
        todo!()
    }

    unsafe fn destroy_query_pool(&self, _: ()) {
        todo!()
    }

    unsafe fn get_query_pool_results(
        &self,
        _: &(),
        _: Range<Id>,
        _: &mut [u8],
        _: Stride,
        _: ResultFlags,
    ) -> Result<bool, WaitError> {
        todo!()
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
        todo!()
    }

    unsafe fn free_memory(&self, _memory: MemoryMock) {}

    unsafe fn destroy_shader_module(&self, _: ()) {}

    unsafe fn destroy_render_pass(&self, _: ()) {}

    unsafe fn destroy_pipeline_layout(&self, _: ()) {}

    unsafe fn destroy_graphics_pipeline(&self, _: ()) {}

    unsafe fn destroy_compute_pipeline(&self, _: ()) {
        todo!()
    }
    unsafe fn destroy_framebuffer(&self, _: ()) {}

    unsafe fn destroy_buffer(&self, _: BufferMock) {}

    unsafe fn destroy_buffer_view(&self, _: ()) {
        todo!()
    }

    unsafe fn destroy_image(&self, _: ImageMock) {}

    unsafe fn destroy_image_view(&self, _: ()) {}

    unsafe fn destroy_sampler(&self, _: ()) {}

    unsafe fn destroy_descriptor_pool(&self, _: DescriptorPoolMock) {}

    unsafe fn destroy_descriptor_set_layout(&self, _: DescriptorSetLayoutMock) {}

    unsafe fn destroy_fence(&self, _: ()) {}

    unsafe fn destroy_semaphore(&self, _: ()) {}

    unsafe fn destroy_event(&self, _: ()) {
        todo!()
    }

    fn wait_idle(&self) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn set_image_name(&self, _: &mut ImageMock, _: &str) {
        todo!()
    }

    unsafe fn set_buffer_name(&self, _: &mut BufferMock, _: &str) {
        todo!()
    }

    unsafe fn set_command_buffer_name(&self, _: &mut CommandBufferMock, _: &str) {
        todo!()
    }

    unsafe fn set_semaphore_name(&self, _: &mut (), _: &str) {
        todo!()
    }

    unsafe fn set_fence_name(&self, _: &mut (), _: &str) {
        todo!()
    }

    unsafe fn set_framebuffer_name(&self, _: &mut (), _: &str) {
        todo!()
    }

    unsafe fn set_render_pass_name(&self, _: &mut (), _: &str) {
        todo!()
    }

    unsafe fn set_descriptor_set_name(&self, set: &mut DescriptorSetMock, name: &str) {
        set.name = name.to_string();
    }

    unsafe fn set_descriptor_set_layout_name(
        &self,
        layout: &mut DescriptorSetLayoutMock,
        name: &str,
    ) {
        layout.name = name.to_string();
    }

    unsafe fn set_pipeline_layout_name(&self, _pipeline_layout: &mut (), _name: &str) {
        todo!()
    }

    unsafe fn reset_fence(&self, _: &mut ()) -> Result<(), OutOfMemory> {
        Ok(())
    }

    unsafe fn wait_for_fence(&self, _: &(), _: u64) -> Result<bool, WaitError> {
        Ok(true)
    }
}
