use {super::{*, Backend}, std::borrow::Borrow};

#[derive(Debug)]
pub struct CommandQueueMock;

impl CommandQueue<Backend> for CommandQueueMock {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(&mut self, _: Submission<Ic, Iw, Is>, _: Option<&mut ()>)
    where
        T: 'a + Borrow<CommandBufferMock>,
        S: 'a + Borrow<()>,
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
