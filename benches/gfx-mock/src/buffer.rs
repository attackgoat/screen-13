#[derive(Debug)]
pub struct BufferMock {
    pub(crate) size: u64,
}

impl BufferMock {
    pub fn new(size: u64) -> Self {
        BufferMock { size }
    }
}
