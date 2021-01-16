use {
    super::*,
    std::{borrow::Borrow, ops::Range},
};

#[derive(Debug)]
pub struct CommandBufferMock;

impl CommandBuffer<BackendMock> for CommandBufferMock {
    unsafe fn begin(
        &mut self,
        _: CommandBufferFlags,
        _: CommandBufferInheritanceInfo<BackendMock>,
    ) {
    }

    unsafe fn finish(&mut self) {}

    unsafe fn reset(&mut self, _: bool) {
        todo!()
    }

    unsafe fn pipeline_barrier<'a, T>(&mut self, _: Range<PipelineStage>, _: Dependencies, _: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'a, BackendMock>>,
    {
    }

    unsafe fn fill_buffer(&mut self, _: &BufferMock, _: SubRange, _: u32) {
        todo!()
    }

    unsafe fn update_buffer(&mut self, _: &BufferMock, _: Offset, _: &[u8]) {
        todo!()
    }

    unsafe fn clear_image<T>(&mut self, _: &ImageMock, _: Layout, _: ClearValue, _: T) {
        todo!()
    }

    unsafe fn clear_attachments<T, U>(&mut self, _: T, _: U) {
        todo!()
    }

    unsafe fn resolve_image<T>(
        &mut self,
        _: &ImageMock,
        _: Layout,
        _: &ImageMock,
        _: Layout,
        _: T,
    ) {
        todo!()
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
        todo!()
    }

    unsafe fn bind_index_buffer(&mut self, _: &BufferMock, _: SubRange, _: IndexType) {
        todo!()
    }

    unsafe fn bind_vertex_buffers<I, T>(&mut self, _: u32, _: I) {}

    unsafe fn set_viewports<T>(&mut self, _: u32, _: T) {}

    unsafe fn set_scissors<T>(&mut self, _: u32, _: T) {}

    unsafe fn set_stencil_reference(&mut self, _: Face, _: StencilValue) {
        todo!()
    }

    unsafe fn set_stencil_read_mask(&mut self, _: Face, _: StencilValue) {
        todo!()
    }

    unsafe fn set_stencil_write_mask(&mut self, _: Face, _: StencilValue) {
        todo!()
    }

    unsafe fn set_blend_constants(&mut self, _: ColorValue) {
        todo!()
    }

    unsafe fn set_depth_bounds(&mut self, _: Range<f32>) {
        todo!()
    }

    unsafe fn set_line_width(&mut self, _: f32) {
        todo!()
    }

    unsafe fn set_depth_bias(&mut self, _: pso::DepthBias) {
        todo!()
    }

    unsafe fn begin_render_pass<'a, T>(&mut self, _: &(), _: &(), _: Rect, _: T, _: SubpassContents)
    where
        T: IntoIterator<Item = RenderAttachmentInfo<'a, BackendMock>>,
    {
    }

    unsafe fn next_subpass(&mut self, _: SubpassContents) {
        todo!()
    }

    unsafe fn end_render_pass(&mut self) {}

    unsafe fn bind_graphics_pipeline(&mut self, _: &()) {}

    unsafe fn bind_graphics_descriptor_sets<I, J>(&mut self, _: &(), _: usize, _: I, _: J) {
        // Do nothing
    }

    unsafe fn bind_compute_pipeline(&mut self, _: &()) {
        todo!()
    }

    unsafe fn bind_compute_descriptor_sets<I, J>(&mut self, _: &(), _: usize, _: I, _: J) {
        // Do nothing
    }

    unsafe fn dispatch(&mut self, _: WorkGroupCount) {
        todo!()
    }

    unsafe fn dispatch_indirect(&mut self, _: &BufferMock, _: Offset) {
        todo!()
    }

    unsafe fn copy_buffer<T>(&mut self, _: &BufferMock, _: &BufferMock, _: T) {
        todo!()
    }

    unsafe fn copy_image<T>(&mut self, _: &ImageMock, _: Layout, _: &ImageMock, _: Layout, _: T) {
        todo!()
    }

    unsafe fn copy_buffer_to_image<T>(&mut self, _: &BufferMock, _: &ImageMock, _: Layout, _: T) {}

    unsafe fn copy_image_to_buffer<T>(&mut self, _: &ImageMock, _: Layout, _: &BufferMock, _: T) {
        todo!()
    }

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

    unsafe fn draw_mesh_tasks(&mut self, _: TaskCount, _: TaskCount) {
        todo!()
    }

    unsafe fn draw_mesh_tasks_indirect(
        &mut self,
        _: &BufferMock,
        _: Offset,
        _: DrawCount,
        _: Stride,
    ) {
        todo!()
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
        todo!()
    }

    unsafe fn set_event(&mut self, _: &(), _: PipelineStage) {
        todo!()
    }

    unsafe fn reset_event(&mut self, _: &(), _: PipelineStage) {
        todo!()
    }

    unsafe fn wait_events<'a, I, J>(&mut self, _: I, _: Range<PipelineStage>, _: J)
    where
        J: IntoIterator,
        J::Item: Borrow<Barrier<'a, BackendMock>>,
    {
        todo!()
    }

    unsafe fn begin_query(&mut self, _: Query<BackendMock>, _: ControlFlags) {
        todo!()
    }

    unsafe fn end_query(&mut self, _: Query<BackendMock>) {
        todo!()
    }

    unsafe fn reset_query_pool(&mut self, _: &(), _: Range<Id>) {
        todo!()
    }

    unsafe fn copy_query_pool_results(
        &mut self,
        _: &(),
        _: Range<Id>,
        _: &BufferMock,
        _: Offset,
        _: Stride,
        _: ResultFlags,
    ) {
        todo!()
    }

    unsafe fn write_timestamp(&mut self, _: PipelineStage, _: Query<BackendMock>) {
        todo!()
    }

    unsafe fn push_graphics_constants(&mut self, _: &(), _: ShaderStageFlags, _: u32, _: &[u32]) {
        todo!()
    }

    unsafe fn push_compute_constants(&mut self, _: &(), _: u32, _: &[u32]) {
        todo!()
    }

    unsafe fn execute_commands<'a, T, I>(&mut self, _: I)
    where
        T: 'a + Borrow<CommandBufferMock>,
        I: IntoIterator<Item = &'a T>,
    {
        todo!()
    }

    unsafe fn insert_debug_marker(&mut self, _: &str, _: u32) {
        todo!()
    }
    unsafe fn begin_debug_marker(&mut self, _: &str, _: u32) {
        todo!()
    }
    unsafe fn end_debug_marker(&mut self) {
        todo!()
    }
}
