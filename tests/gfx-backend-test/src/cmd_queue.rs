use {
    super::{Backend, *},
};

#[derive(Debug)]
pub struct CommandQueueMock;

impl CommandQueue<Backend> for CommandQueueMock {
    unsafe fn submit<'a, Ic, Iw, Is>(
        &mut self,
        command_buffers: Ic,
        wait_semaphores: Iw,
        signal_semaphores: Is,
        fence: Option<&mut <Backend as gfx_hal::Backend>::Fence>,
    ) where
        Ic: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::CommandBuffer>,
        Ic::IntoIter: ExactSizeIterator,
        Iw: IntoIterator<Item = (&'a <Backend as gfx_hal::Backend>::Semaphore, PipelineStage)>,
        Iw::IntoIter: ExactSizeIterator,
        Is: IntoIterator<Item = &'a <Backend as gfx_hal::Backend>::Semaphore>,
        Is::IntoIter: ExactSizeIterator
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
