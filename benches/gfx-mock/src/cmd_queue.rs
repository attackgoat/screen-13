use gfx_hal::{
    adapter::*, buffer::*, command::*, device::*, format::*, image::*, memory::*, pass::*, pool::*,
    pso::*, query::*, queue::*, window::*, *,
};

#[derive(Debug)]
pub struct CommandQueueMock;

impl CommandQueue<Backend> for CommandQueueMock {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        _: Submission<Ic, Iw, Is>,
        _: Option<&mut ()>,
    ) where
        T: 'a + Borrow<CommandBuffer>,
        S: 'a + Borrow<()>,
    {
    }

    unsafe fn present(
        &mut self,
        _surface: &mut Surface,
        _image: SwapchainImage,
        _wait_semaphore: Option<&mut ()>,
    ) -> Result<Option<window::Suboptimal>, window::PresentError> {
        Ok(None)
    }

    fn wait_idle(&mut self) -> Result<(), device::OutOfMemory> {
        unimplemented!("{}", NOT_SUPPORTED_MESSAGE)
    }
}
