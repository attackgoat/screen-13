use super::{*, Backend};

#[derive(Debug)]
pub struct CommandPoolMock;

impl CommandPool<Backend> for CommandPoolMock {
    unsafe fn allocate_one(&mut self, level: Level) -> CommandBufferMock {
        assert_eq!(
            level,
            Level::Primary,
            "Only primary command buffers are supported"
        );

        CommandBufferMock
    }

    unsafe fn reset(&mut self, _: bool) {}

    unsafe fn free<I>(&mut self, _: I) {
        todo!()
    }
}
