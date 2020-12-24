use {
    super::{
        align_down, align_up,
        driver::{Buffer, Device, Driver, Memory},
    },
    gfx_hal::{
        adapter::PhysicalDevice as _,
        buffer::{Access, SubRange, Usage},
        command::{BufferCopy, CommandBuffer as _},
        device::{Device as _, MapError, OutOfMemory},
        memory::{Barrier, Dependencies, Properties, Segment},
        pso::PipelineStage,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        borrow::Borrow,
        iter::once,
        ops::{Deref, DerefMut, Range},
        slice::{
            from_raw_parts as slice_from_raw_parts, from_raw_parts_mut as slice_from_raw_parts_mut,
        },
        u64,
    },
};

/// An iterator to allow incoming `Iterator`'s of `CopyRange` to output `Barrier::Buffer` for the destination region.
struct BarrierIter<'a, T>
where
    T: Iterator,
    T::Item: Borrow<CopyRange>,
{
    ranges: T,
    states: Range<Access>,
    target: &'a <_Backend as Backend>::Buffer,
}

impl<'a, T> Iterator for BarrierIter<'a, T>
where
    T: Iterator,
    T::Item: Borrow<CopyRange>,
{
    type Item = Barrier<'a, _Backend>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.ranges.next() {
            Some(range) => {
                let r = range.borrow();

                Some(Barrier::Buffer {
                    families: None,
                    range: SubRange {
                        offset: r.dst,
                        size: Some(r.src.end - r.src.start),
                    },
                    states: self.states.clone(),
                    target: self.target,
                })
            }
            _ => None,
        }
    }
}

/// An iterator to allow incoming `Iterator`'s of `CopyRange` to output `BufferCopy` instead.
struct BufferCopyIter<T>(T)
where
    T: ExactSizeIterator, // TODO: Can I drop these specifications and keep the impls? Test
    T::Item: Borrow<CopyRange>;

impl<T> ExactSizeIterator for BufferCopyIter<T>
where
    T: ExactSizeIterator,
    T::Item: Borrow<CopyRange>,
{
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Iterator for BufferCopyIter<T>
where
    T: ExactSizeIterator,
    T::Item: Borrow<CopyRange>,
{
    type Item = BufferCopy;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(range) => {
                let r = range.borrow();

                Some(BufferCopy {
                    dst: r.dst,
                    size: r.src.end - r.src.start,
                    src: r.src.start,
                })
            }
            _ => None,
        }
    }
}

struct BufferMemory {
    buf: Buffer,
    mem: Memory,
}

#[derive(Clone)]
pub struct CopyRange {
    pub dst: u64,
    pub src: Range<u64>,
}

/// A buffer type which automates the tasks related to transferring bytes to the graphics device.
/// Data can be read from, written to, or copied within the graphics device, and mapped in order
/// to gain access to the raw bytes.
pub struct Data {
    access_mask: Access,
    capacity: u64,
    driver: Driver,
    pipeline_stage: PipelineStage,
    staging: Option<BufferMemory>,
    storage: BufferMemory,
}

impl Data {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        mut capacity: u64,
        usage: Usage,
    ) -> Self {
        assert_ne!(capacity, 0);

        // Pre-align the capacity so the entire requested capacity can be mapped later (mapping must be in atom sized units)
        let non_coherent_atom_size = Device::gpu(&driver.as_ref().borrow())
            .limits()
            .non_coherent_atom_size;
        capacity = align_up(capacity, non_coherent_atom_size as _);

        let mut storage_buf = Buffer::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            Usage::TRANSFER_DST | Usage::TRANSFER_SRC | usage,
            capacity,
        );
        let (storage_mem, staging) = {
            let device = driver.as_ref().borrow();

            // Get the main storage buffer memory requirements and find out if we're using a unified memory architecutre
            let storage_req = unsafe { device.get_buffer_requirements(&storage_buf) };
            let (storage_mem_ty, is_uma) = if let Some(mem_ty) = Device::mem_ty(
                &device,
                storage_req.type_mask,
                Properties::CPU_VISIBLE | Properties::DEVICE_LOCAL,
            ) {
                (mem_ty, true)
            } else {
                let mem_ty =
                    Device::mem_ty(&device, storage_req.type_mask, Properties::DEVICE_LOCAL)
                        .unwrap();
                (mem_ty, false)
            };
            let storage_mem = Memory::new(Driver::clone(&driver), storage_mem_ty, storage_req.size);

            // Bind the main storage memory
            unsafe {
                device
                    .bind_buffer_memory(&storage_mem, 0, &mut storage_buf)
                    .unwrap();
            }

            // Optionally create a staging buffer on non-unified memory architectures
            let staging = if is_uma {
                None
            } else {
                let mut staging_buf = Buffer::new(
                    #[cfg(debug_assertions)]
                    name,
                    Driver::clone(&driver),
                    Usage::TRANSFER_DST | Usage::TRANSFER_SRC,
                    capacity,
                );
                let staging_req = unsafe { device.get_buffer_requirements(&staging_buf) };
                let staging_mem_ty =
                    Device::mem_ty(&device, staging_req.type_mask, Properties::CPU_VISIBLE)
                        .unwrap();
                let staging_mem =
                    Memory::new(Driver::clone(&driver), staging_mem_ty, staging_req.size);

                // Bind the optional staging memory
                unsafe {
                    device
                        .bind_buffer_memory(&staging_mem, 0, &mut staging_buf)
                        .unwrap();
                }

                Some(BufferMemory {
                    buf: staging_buf,
                    mem: staging_mem,
                })
            };

            (storage_mem, staging)
        };
        let storage = BufferMemory {
            buf: storage_buf,
            mem: storage_mem,
        };

        Self {
            access_mask: Access::empty(),
            capacity,
            driver,
            pipeline_stage: PipelineStage::TOP_OF_PIPE,
            staging,
            storage,
        }
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Copies a portion within the graphics device to another portion.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn copy_range(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        range: CopyRange,
    ) {
        self.copy_ranges(cmd_buf, pipeline_stage, access_mask, &[range])
    }

    /// Copies portions within the graphics device to other portions.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn copy_ranges<R>(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        ranges: R,
    ) where
        R: Copy + IntoIterator,
        R::Item: Borrow<CopyRange>,
        R::IntoIter: ExactSizeIterator,
    {
        let copies = BufferCopyIter(ranges.into_iter());
        cmd_buf.copy_buffer(&self.storage.buf, &self.storage.buf, copies);

        let barriers = BarrierIter {
            ranges: ranges.into_iter(),
            states: self.access_mask..access_mask,
            target: &*self.storage.buf,
        };
        cmd_buf.pipeline_barrier(
            self.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            barriers,
        );

        self.access_mask = access_mask;
        self.pipeline_stage = pipeline_stage;
    }

    /// Provides read-only access to the raw bytes.
    pub fn map(&mut self) -> Result<Mapping, MapError> {
        self.map_range(0..self.capacity)
    }

    /// Provides read-only access to a portion of the raw bytes.
    pub fn map_range(&mut self, range: Range<u64>) -> Result<Mapping, MapError> {
        self.map_memory(range)
    }

    /// Provides mutable access to the raw bytes.
    pub fn map_mut(&mut self) -> Result<Mapping, MapError> {
        self.map_range_mut(0..self.capacity)
    }

    /// Provides mutable access to a portion of the raw bytes.
    pub fn map_range_mut(&mut self, range: Range<u64>) -> Result<Mapping, MapError> {
        self.map_memory(range)
    }

    // Note: mut because "It is an application error to call vkMapMemory on a memory object that is already host mapped."
    fn map_memory(&mut self, range: Range<u64>) -> Result<Mapping, MapError> {
        assert!(range.start < range.end);
        assert!(range.end <= self.capacity);

        let driver = Driver::clone(&self.driver);
        let mem = self
            .staging
            .as_ref()
            .map(|staging| &staging.mem)
            .unwrap_or(&self.storage.mem);

        unsafe { Mapping::new(driver, mem, range) }
    }

    /// Reads everything from the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn read(&mut self, cmd_buf: &mut <_Backend as Backend>::CommandBuffer) {
        self.read_range(cmd_buf, 0..self.capacity)
    }

    /// Reads a portion from the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn read_range(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        range: Range<u64>,
    ) {
        self.read_ranges(cmd_buf, &[range])
    }

    /// Reads portions from the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn read_ranges<R>(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        ranges: R,
    ) where
        R: Copy + IntoIterator,
        R::Item: Borrow<Range<u64>>,
        R::IntoIter: ExactSizeIterator,
    {
        // This is a no-op on unified memory architectures
        if let Some(staging) = &self.staging {
            let ranges = RangeAdapter(ranges);
            let copies = BufferCopyIter(ranges.into_iter());
            cmd_buf.copy_buffer(&self.storage.buf, &staging.buf, copies);
        }
    }

    /// Sets a descriptive name for debugging which can be seen with API tracing tools such as RenderDoc.
    #[cfg(debug_assertions)]
    pub fn set_name(&mut self, name: &str) {
        if let Some(staging) = &mut self.staging {
            Buffer::set_name(&mut staging.buf, name);
        }

        Buffer::set_name(&mut self.storage.buf, name);
    }

    /// Transfers a portion within the graphics device to another instance using a copy.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn transfer_range(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        other: &mut Self,
        range: CopyRange,
    ) {
        self.transfer_ranges(cmd_buf, other, &[range])
    }

    /// Transfers portions within the graphics device to another instance using a copy.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn transfer_ranges<R>(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        other: &mut Self,
        ranges: R,
    ) where
        R: Copy + IntoIterator,
        R::Item: Borrow<CopyRange>,
        R::IntoIter: ExactSizeIterator,
    {
        let copies = BufferCopyIter(ranges.into_iter());
        cmd_buf.copy_buffer(&self.storage.buf, &other.storage.buf, copies);
    }

    /// Writes everything to the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn write(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        self.write_range(cmd_buf, pipeline_stage, access_mask, 0..self.capacity)
    }

    /// Writes a portion to the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn write_range(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        range: Range<u64>,
    ) {
        self.write_ranges(cmd_buf, pipeline_stage, access_mask, &[range])
    }

    /// Writes portions to the graphics device.
    ///
    /// # Safety
    ///
    /// The provided command buffer must be ready to record.
    pub unsafe fn write_ranges<R>(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        ranges: R,
    ) where
        R: Copy + IntoIterator,
        R::Item: Borrow<Range<u64>>,
        R::IntoIter: ExactSizeIterator,
    {
        let ranges = RangeAdapter(ranges);
        if let Some(staging) = &self.staging {
            let copies = BufferCopyIter(ranges.into_iter());
            cmd_buf.copy_buffer(&staging.buf, &self.storage.buf, copies);
        }

        let barriers = BarrierIter {
            ranges: ranges.into_iter(),
            states: self.access_mask..access_mask,
            target: &*self.storage.buf,
        };
        cmd_buf.pipeline_barrier(
            self.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            barriers,
        );

        self.access_mask = access_mask;
        self.pipeline_stage = pipeline_stage;
    }
}

impl AsRef<<_Backend as Backend>::Buffer> for Data {
    fn as_ref(&self) -> &<_Backend as Backend>::Buffer {
        &*self.storage.buf
    }
}

pub struct Mapping<'m> {
    driver: Driver,
    flushed: bool,
    len: usize,
    mapped_mem: (&'m <_Backend as Backend>::Memory, Segment),
    ptr: *mut u8,
}

impl<'m> Mapping<'m> {
    /// # Safety
    ///
    /// The given memory must not be mapped and contain the given range.
    unsafe fn new(
        driver: Driver,
        mem: &'m <_Backend as Backend>::Memory,
        range: Range<u64>,
    ) -> Result<Self, MapError> {
        assert_ne!(range.end, 0);
        assert!(range.start < range.end);

        // Mapped host memory ranges must be in multiples of atom size; so we align to a possibly larger window
        let non_coherent_atom_size = Device::gpu(&driver.as_ref().borrow())
            .limits()
            .non_coherent_atom_size;
        let offset = align_down(range.start, non_coherent_atom_size as _);
        let size = align_up(range.end - range.start, non_coherent_atom_size as _);

        // debug!(
        //     "Request {}..{} got {}..{} (ncas {})",
        //     range.start,
        //     range.end,
        //     offset,
        //     offset + size,
        //     non_coherent_atom_size
        // );

        let segment = Segment {
            offset,
            size: Some(size),
        };
        let (mapped_mem, ptr) = {
            let device = driver.as_ref().borrow();
            let mapped_mem = (mem, segment.clone());
            let ptr = device
                .map_memory(mem, segment)?
                .offset((range.start - offset) as _);
            device.invalidate_mapped_memory_ranges(once(&mapped_mem))?;

            (mapped_mem, ptr)
        };

        Ok(Self {
            driver,
            flushed: true,
            len: (range.end - range.start) as _,
            mapped_mem,
            ptr,
        })
    }

    /// Releases the mapped memory back to the device, only needs to be called if this a mutable mapping.
    ///
    /// Remarks: Failing to call this function before dropping a Mapping may cause a panic.
    pub fn flush(mapping: &mut Self) -> Result<(), OutOfMemory> {
        if !mapping.flushed {
            mapping.flushed = true;

            let device = mapping.driver.as_ref().borrow();

            unsafe {
                device.flush_mapped_memory_ranges(once(&mapping.mapped_mem))?;
            }
        }

        Ok(())
    }
}

impl Deref for Mapping<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { slice_from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for Mapping<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Set the flag because we must tell the device this segment has been written to!
        self.flushed = false;

        unsafe { slice_from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for Mapping<'_> {
    fn drop(&mut self) {
        // This will panic if it fails; call `flush()` first to prevent this!
        Self::flush(self).unwrap();

        let device = self.driver.as_ref().borrow();

        unsafe {
            device.unmap_memory(self.mapped_mem.0);
        }
    }
}

/// An adapter to allow incoming `IntoIter`'s of `Range` to output `CopyRange` instead.
#[derive(Clone, Copy)]
struct RangeAdapter<T>(T)
where
    T: Copy + IntoIterator,
    T::Item: Borrow<Range<u64>>,
    T::IntoIter: ExactSizeIterator;

impl<T> IntoIterator for RangeAdapter<T>
where
    T: Copy + IntoIterator,
    T::Item: Borrow<Range<u64>>,
    T::IntoIter: ExactSizeIterator,
{
    type IntoIter = RangeIter<T::IntoIter>;
    type Item = CopyRange;

    fn into_iter(self) -> Self::IntoIter {
        RangeIter(self.0.into_iter())
    }
}

/// An iterator to allow incoming `Iterator`'s of `Range` to output `CopyRange` instead.
#[derive(Clone, Copy)]
struct RangeIter<T>(T)
where
    T: ExactSizeIterator,
    T::Item: Borrow<Range<u64>>;

impl<T> ExactSizeIterator for RangeIter<T>
where
    T: ExactSizeIterator,
    T::Item: Borrow<Range<u64>>,
{
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Iterator for RangeIter<T>
where
    T: ExactSizeIterator,
    T::Item: Borrow<Range<u64>>,
{
    type Item = CopyRange;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(range) => {
                let range = range.borrow().clone();

                Some(CopyRange {
                    dst: range.start,
                    src: range,
                })
            }
            _ => None,
        }
    }
}
