use {
    super::{Backend, *},
    std::{borrow::Borrow, ops::Range},
};

#[derive(Debug)]
pub struct CommandBufferMock;

impl CommandBuffer<Backend> for CommandBufferMock {
    unsafe fn begin(&mut self, _: CommandBufferFlags, _: CommandBufferInheritanceInfo<Backend>) {}

    unsafe fn finish(&mut self) {}

    unsafe fn reset(&mut self, _: bool) {}

    unsafe fn pipeline_barrier<'a, T>(&mut self, _: Range<PipelineStage>, _: Dependencies, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'a, Backend>>,
    {
    }

    unsafe fn fill_buffer(&mut self, _: &BufferMock, _: SubRange, _: u32) {}

    unsafe fn update_buffer(&mut self, _: &BufferMock, _: Offset, _: &[u8]) {}

    unsafe fn clear_image<T>(&mut self, _: &ImageMock, _: Layout, _: ClearValue, _: T) {}

    unsafe fn clear_attachments<T, U>(&mut self, _: T, _: U) {}

    unsafe fn resolve_image<T>(
        &mut self,
        _: &ImageMock,
        _: Layout,
        _: &ImageMock,
        _: Layout,
        _: T,
    ) {
    }

    unsafe fn blit_image<T>(
        &mut self,
        _: &ImageMock,
        _: Layout,
        _: &ImageMock,
        _: Layout,
        _: Filter,
        _: T,
    ) {
    }

    unsafe fn bind_index_buffer(&mut self, _: &BufferMock, _: SubRange, _: IndexType) {}

    unsafe fn bind_vertex_buffers<'a, T>(&mut self, _first_binding: BufferIndex, _buffers: T)
    where
        T: IntoIterator<Item = (&'a <Backend as gfx_hal::Backend>::Buffer, SubRange)>,
    {
    }

    unsafe fn set_viewports<T>(&mut self, _: u32, _: T) {}

    unsafe fn set_scissors<T>(&mut self, _: u32, _: T) {}

    unsafe fn set_stencil_reference(&mut self, _: Face, _: StencilValue) {}

    unsafe fn set_stencil_read_mask(&mut self, _: Face, _: StencilValue) {}

    unsafe fn set_stencil_write_mask(&mut self, _: Face, _: StencilValue) {}

    unsafe fn set_blend_constants(&mut self, _: ColorValue) {}

    unsafe fn set_depth_bounds(&mut self, _: Range<f32>) {}

    unsafe fn set_line_width(&mut self, _: f32) {}

    unsafe fn set_depth_bias(&mut self, _: pso::DepthBias) {}

    unsafe fn begin_render_pass<'a, T>(&mut self, _: &(), _: &(), _: Rect, _: T, _: SubpassContents)
    where
        T: IntoIterator<Item = RenderAttachmentInfo<'a, Backend>>,
    {
    }

    unsafe fn next_subpass(&mut self, _: SubpassContents) {}

    unsafe fn end_render_pass(&mut self) {}

    unsafe fn bind_graphics_pipeline(&mut self, _: &()) {}

    unsafe fn bind_graphics_descriptor_sets<'a, I, J>(
        &mut self,
        _: &<Backend as gfx_hal::Backend>::PipelineLayout,
        _: usize,
        _: I,
        _: J,
    ) where
        I: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::DescriptorSet>,
        J: IntoIterator<Item = DescriptorSetOffset>,
    {
    }

    unsafe fn bind_compute_pipeline(&mut self, _: &()) {}

    unsafe fn bind_compute_descriptor_sets<'a, I, J>(
        &mut self,
        _: &<Backend as gfx_hal::Backend>::PipelineLayout,
        _: usize,
        _: I,
        _: J,
    ) where
        I: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::DescriptorSet>,
        J: IntoIterator<Item = DescriptorSetOffset>,
    {
    }

    unsafe fn dispatch(&mut self, _: WorkGroupCount) {}

    unsafe fn dispatch_indirect(&mut self, _: &BufferMock, _: Offset) {}

    unsafe fn copy_buffer<T>(&mut self, _: &BufferMock, _: &BufferMock, _: T) {}

    unsafe fn copy_image<T>(&mut self, _: &ImageMock, _: Layout, _: &ImageMock, _: Layout, _: T) {}

    unsafe fn copy_buffer_to_image<T>(&mut self, _: &BufferMock, _: &ImageMock, _: Layout, _: T) {}

    unsafe fn copy_image_to_buffer<T>(&mut self, _: &ImageMock, _: Layout, _: &BufferMock, _: T) {}

    unsafe fn draw(&mut self, _: Range<VertexCount>, _: Range<InstanceCount>) {}

    unsafe fn draw_indexed(
        &mut self,
        _: Range<IndexCount>,
        _: VertexOffset,
        _: Range<InstanceCount>,
    ) {
    }

    unsafe fn draw_indirect(&mut self, _: &BufferMock, _: Offset, _: DrawCount, _: Stride) {}

    unsafe fn draw_indexed_indirect(&mut self, _: &BufferMock, _: Offset, _: DrawCount, _: Stride) {
    }

    unsafe fn draw_indirect_count(
        &mut self,
        _: &BufferMock,
        _: Offset,
        _: &BufferMock,
        _: Offset,
        _: u32,
        _: Stride,
    ) {
    }

    unsafe fn draw_indexed_indirect_count(
        &mut self,
        _: &BufferMock,
        _: Offset,
        _: &BufferMock,
        _: Offset,
        _: u32,
        _: Stride,
    ) {
    }

    unsafe fn draw_mesh_tasks(&mut self, _: TaskCount, _: TaskCount) {}

    unsafe fn draw_mesh_tasks_indirect(
        &mut self,
        _: &BufferMock,
        _: Offset,
        _: DrawCount,
        _: Stride,
    ) {
    }

    unsafe fn draw_mesh_tasks_indirect_count(
        &mut self,
        _: &BufferMock,
        _: Offset,
        _: &BufferMock,
        _: Offset,
        _: u32,
        _: Stride,
    ) {
    }

    unsafe fn set_event(&mut self, _: &(), _: PipelineStage) {}

    unsafe fn reset_event(&mut self, _: &(), _: PipelineStage) {}

    unsafe fn wait_events<'a, I, J>(&mut self, _: I, _: Range<PipelineStage>, _: J)
    where
        J: IntoIterator,
        J::Item: Borrow<Barrier<'a, Backend>>,
    {
    }

    unsafe fn begin_query(&mut self, _: Query<Backend>, _: ControlFlags) {}

    unsafe fn end_query(&mut self, _: Query<Backend>) {}

    unsafe fn reset_query_pool(&mut self, _: &(), _: Range<Id>) {}

    unsafe fn copy_query_pool_results(
        &mut self,
        _: &(),
        _: Range<Id>,
        _: &BufferMock,
        _: Offset,
        _: Stride,
        _: ResultFlags,
    ) {
    }

    unsafe fn write_timestamp(&mut self, _: PipelineStage, _: Query<Backend>) {}

    unsafe fn push_graphics_constants(&mut self, _: &(), _: ShaderStageFlags, _: u32, _: &[u32]) {}

    unsafe fn push_compute_constants(&mut self, _: &(), _: u32, _: &[u32]) {}

    unsafe fn execute_commands<'a, T>(&mut self, _cmd_buffers: T)
    where
        T: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::CommandBuffer>,
    {
    }

    unsafe fn insert_debug_marker(&mut self, _: &str, _: u32) {}

    unsafe fn begin_debug_marker(&mut self, _: &str, _: u32) {}

    unsafe fn end_debug_marker(&mut self) {}
}
