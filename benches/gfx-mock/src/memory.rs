use {super::*, std::{cell::UnsafeCell, convert::TryInto}};

#[derive(Debug)]
pub struct MemoryMock {
    memory_type: MemoryTypeId,
    size: u64,
    data: UnsafeCell<Box<[u8]>>,
}

impl MemoryMock {
    pub fn allocate(memory_type: MemoryTypeId, size: u64) -> Result<Self, AllocationError> {
        assert_eq!(memory_type.0, 0, "We only support one memory type");

        let data = {
            let size = size
                .                            try_into()
                // If we're on 32-bit and the given size is greater than 2^32,
                // we certainly can't allocate it.
                .map_err(|_| AllocationError::OutOfMemory(OutOfMemory::Host))?;

            vec![0u8; size].into_boxed_slice()
        };
        let memory = Self {
            memory_type,
            size,
            data: UnsafeCell::new(data),
        };

        Ok(memory)
    }

    pub fn map(&self, segment: Segment) -> Result<*mut u8, MapError> {
        if segment.offset >= self.size {
            return Err(MapError::OutOfBounds);
        }
        if let Some(size) = segment.size {
            if segment.offset + size > self.size {
                return Err(MapError::OutOfBounds);
            }
        }

        let data = unsafe { &mut *self.data.get() };
        Ok(data.as_mut_ptr())
    }
}

unsafe impl Sync for MemoryMock {}
