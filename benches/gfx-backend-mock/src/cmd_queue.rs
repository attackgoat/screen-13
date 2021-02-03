use {
    super::{Backend, *},
};

#[derive(Debug)]
pub struct CommandQueueMock;

impl CommandQueue<Backend> for CommandQueueMock {
    unsafe fn submit<'a, Ic, Iw, Is>(
        &mut self,
        _command_buffers: Ic,
        _wait_semaphores: Iw,
        _signal_semaphores: Is,
        _fence: Option<&mut <Backend as gfx_hal::Backend>::Fence>,
    ) where
        Ic: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::CommandBuffer>,
        Iw: IntoIterator<Item = (&'a <Backend as gfx_hal::Backend>::Semaphore, PipelineStage)>,
        Is: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::Semaphore>,
    {
    }

    unsafe fn present(
        &mut self,
        _surface: &mut SurfaceMock,
        _image: SwapchainImageMock,
        _wait_semaphore: Option<&mut ()>,
    ) -> Result<Option<Suboptimal>, PresentError> {
        Ok(None)
    }

    fn wait_idle(&mut self) -> Result<(), OutOfMemory> {
        todo!()
    }
}
