use {
    super::driver::{Buffer, Driver, Memory, Semaphore},
    gfx_hal::{
        buffer::{Access, SubRange, Usage},
        command::{BufferCopy, CommandBuffer as _},
        device::Device,
        memory::{Barrier, Dependencies, Properties, Segment},
        pso::PipelineStage,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        cell::RefCell,
        iter::once,
        ops::{Deref, DerefMut, Range},
        rc::Rc,
        slice::{
            from_raw_parts as slice_from_raw_parts, from_raw_parts_mut as slice_from_raw_parts_mut,
        },
        u64,
    },
};

#[derive(Debug)]
pub struct Data {
    capacity: u64,
    cpu_buf: (Buffer, RefCell<State>),
    driver: Driver,
    gpu_buf: (Buffer, RefCell<State>),
    op: Option<(Rc<Semaphore>, PipelineStage)>,
}

impl Data {
    pub fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        len: u64,
        usage: Usage,
    ) -> Self {
        let cpu_buf = Buffer::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            Usage::TRANSFER_SRC,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            len,
        );
        let gpu_buf = Buffer::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            Usage::TRANSFER_DST | Usage::TRANSFER_SRC | usage,
            Properties::DEVICE_LOCAL,
            len,
        );
        let cpu_state = RefCell::new(State {
            access_mask: Access::empty(),
            pipeline_stage: PipelineStage::TOP_OF_PIPE,
        });
        let gpu_state = RefCell::new(State {
            access_mask: Access::empty(),
            pipeline_stage: PipelineStage::TOP_OF_PIPE,
        });

        Self {
            capacity: len,
            cpu_buf: (cpu_buf, cpu_state),
            driver,
            gpu_buf: (gpu_buf, gpu_state),
            op: Default::default(),
        }
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    unsafe fn copy_range(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        range: Range<u64>,
        cpu: bool,
    ) {
        let (src, dst) = if cpu {
            (&self.cpu_buf, &self.gpu_buf)
        } else {
            (&self.gpu_buf, &self.cpu_buf)
        };

        let mut src_state = src.1.borrow_mut();
        cmd_buf.pipeline_barrier(
            src_state.pipeline_stage..PipelineStage::TRANSFER,
            Dependencies::empty(),
            &[Barrier::Buffer {
                states: src_state.access_mask..Access::TRANSFER_READ,
                target: src.0.as_ref(),
                families: None,
                range: SubRange {
                    offset: range.start,
                    size: Some(range.end - range.start),
                },
            }],
        );

        src_state.access_mask = Access::TRANSFER_READ;
        src_state.pipeline_stage = PipelineStage::TRANSFER;

        cmd_buf.copy_buffer(
            &src.0,
            &dst.0,
            once(BufferCopy {
                dst: range.start,
                size: range.end - range.start,
                src: range.start,
            }),
        );

        let mut dst_state = dst.1.borrow_mut();
        cmd_buf.pipeline_barrier(
            dst_state.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            &[Barrier::Buffer {
                states: dst_state.access_mask..access_mask,
                target: dst.0.as_ref(),
                families: None,
                range: SubRange {
                    offset: range.start,
                    size: Some(range.end - range.start),
                },
            }],
        );

        dst_state.access_mask = access_mask;
        dst_state.pipeline_stage = pipeline_stage;
    }

    /// # Safety
    /// None
    pub unsafe fn copy_cpu(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        len: u64,
    ) {
        self.copy_cpu_range(cmd_buf, pipeline_stage, access_mask, 0..len)
    }

    /// # Safety
    /// None
    pub unsafe fn copy_cpu_range(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        range: Range<u64>,
    ) {
        self.copy_range(cmd_buf, pipeline_stage, access_mask, range, true)
    }

    /// # Safety
    /// None
    pub unsafe fn copy_gpu(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        len: u64,
    ) {
        self.copy_gpu_range(cmd_buf, pipeline_stage, access_mask, 0..len)
    }

    /// # Safety
    /// None
    pub unsafe fn copy_gpu_range(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
        range: Range<u64>,
    ) {
        self.copy_range(cmd_buf, pipeline_stage, access_mask, range, false)
    }

    pub unsafe fn map<'a>(&'a self) -> impl Deref<Target = [u8]> + 'a {
        self.map_mut()
    }

    pub unsafe fn map_range<'a>(&'a self, range: Range<u64>) -> impl Deref<Target = [u8]> + 'a {
        self.map_range_mut(range)
    }

    pub unsafe fn map_mut<'a>(&'a self) -> impl DerefMut<Target = [u8]> + 'a {
        self.map_range_mut(0..self.capacity)
    }

    pub unsafe fn map_range_mut<'a>(
        &'a self,
        range: Range<u64>,
    ) -> impl DerefMut<Target = [u8]> + 'a {
        let device = self.driver.borrow();
        let len = range.end - range.start;
        let mem = self.cpu_buf.0.mem();
        let ptr = device
            .map_memory(
                mem,
                Segment {
                    offset: range.start,
                    size: Some(len),
                },
            )
            .unwrap();

        Mapping {
            driver: Driver::clone(&self.driver),
            len: len as _,
            mem,
            ptr,
        }
    }

    /// # Safety
    /// None
    pub(crate) unsafe fn pipeline_barrier_cpu(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        Self::pipeline_barrier(cmd_buf, &mut self.cpu_buf, pipeline_stage, access_mask);
    }

    /// # Safety
    /// None
    pub(crate) unsafe fn pipeline_barrier_gpu(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        Self::pipeline_barrier(cmd_buf, &mut self.gpu_buf, pipeline_stage, access_mask);
    }

    unsafe fn pipeline_barrier(
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        buf: &mut (Buffer, RefCell<State>),
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        let mut state = buf.1.borrow_mut();
        cmd_buf.pipeline_barrier(
            state.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            &[Barrier::Buffer {
                families: None,
                range: SubRange::WHOLE,
                states: state.access_mask..access_mask,
                target: buf.0.as_ref(),
            }],
        );

        state.access_mask = access_mask;
        state.pipeline_stage = pipeline_stage;
    }
}

impl AsRef<<_Backend as Backend>::Buffer> for Data {
    fn as_ref(&self) -> &<_Backend as Backend>::Buffer {
        self.gpu_buf.0.as_ref()
    }
}

impl Deref for Data {
    type Target = <_Backend as Backend>::Buffer;

    fn deref(&self) -> &Self::Target {
        &*self.gpu_buf.0
    }
}

pub struct Mapping<'m> {
    driver: Driver,
    len: usize,
    mem: &'m Memory,
    ptr: *mut u8,
}

impl Deref for Mapping<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { slice_from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for Mapping<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice_from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for Mapping<'_> {
    fn drop(&mut self) {
        let device = self.driver.borrow();
        unsafe {
            device.unmap_memory(self.mem);
        }
    }
}

#[derive(Debug)]
struct State {
    access_mask: Access,
    pipeline_stage: PipelineStage,
}
