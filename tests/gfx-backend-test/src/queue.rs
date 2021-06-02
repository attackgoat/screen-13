use super::{Backend, *};

#[derive(Debug)]
pub struct CommandQueueMock;

impl CommandQueue<Backend> for CommandQueueMock {
    unsafe fn bind_sparse<'a, Iw, Is, Ibi, Ib, Iii, Io, Ii>(
        &mut self,
        _wait_semaphores: Iw,
        _signal_semaphores: Is,
        _buffer_memory_binds: Ib,
        _image_opaque_memory_binds: Io,
        _image_memory_binds: Ii,
        _device: &<Backend as gfx_hal::Backend>::Device,
        _fence: Option<&<Backend as gfx_hal::Backend>::Fence>,
    ) where
        Ibi: Iterator<Item = &'a SparseBind<&'a <Backend as gfx_hal::Backend>::Memory>>,
        Ib: Iterator<Item = (&'a mut <Backend as gfx_hal::Backend>::Buffer, Ibi)>,
        Iii: Iterator<Item = &'a SparseImageBind<&'a <Backend as gfx_hal::Backend>::Memory>>,
        Io: Iterator<Item = (&'a mut <Backend as gfx_hal::Backend>::Image, Ibi)>,
        Ii: Iterator<Item = (&'a mut <Backend as gfx_hal::Backend>::Image, Iii)>,
        Iw: Iterator<Item = &'a <Backend as gfx_hal::Backend>::Semaphore>,
        Is: Iterator<Item = &'a <Backend as gfx_hal::Backend>::Semaphore>,
    {
    }

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
        Is::IntoIter: ExactSizeIterator,
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

    fn timestamp_period(&self) -> f32 {
        1.0
    }
}
