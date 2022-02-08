use {
    super::{
        is_write_access, Buffer, BufferSubresource, Device, DriverError, Image, ImageSubresource,
        QueueFamily,
    },
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::vk,
    log::trace,
    std::{
        cell::RefCell,
        fmt::Debug,
        ops::{Deref, Range},
        thread::panicking,
    },
    vk_sync::{cmd, AccessType, BufferBarrier, GlobalBarrier, ImageBarrier, ImageLayout},
};

#[derive(Debug)]
pub struct CommandBuffer<P>
where
    P: SharedPointerKind,
{
    cmd_buf: vk::CommandBuffer,
    pub(crate) device: Shared<Device<P>, P>,
    droppables: RefCell<Vec<Box<dyn Debug + 'static>>>,
    pub fence: vk::Fence, // Keeps state because everyone wants this
    pub pool: vk::CommandPool,
}

impl<P> CommandBuffer<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &Shared<Device<P>, P>,
        queue_family: QueueFamily,
    ) -> Result<Self, DriverError> {
        let device = Shared::clone(device);
        let cmd_pool_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::empty())
            .queue_family_index(queue_family.idx);
        let cmd_pool = unsafe {
            device
                .create_command_pool(&cmd_pool_info, None)
                .map_err(|_| DriverError::Unsupported)?
        };
        let cmd_buf_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY);
        let cmd_buf = unsafe {
            device
                .allocate_command_buffers(&cmd_buf_info)
                .map_err(|_| DriverError::Unsupported)?
        }[0];
        let fence = unsafe {
            device
                .create_fence(
                    &vk::FenceCreateInfo::builder()
                        .flags(vk::FenceCreateFlags::SIGNALED)
                        .build(),
                    None,
                )
                .map_err(|_| DriverError::Unsupported)?
        };

        Ok(Self {
            cmd_buf,
            device,
            droppables: RefCell::new(vec![]),
            fence,
            pool: cmd_pool,
        })
    }

    pub fn buffer_barrier(
        this: &Self,
        previous_access: AccessType,
        next_access: AccessType,
        buf: vk::Buffer,
        subresource_range: Option<Range<u64>>,
    ) {
        use std::slice::from_ref;

        let (offset, size) = subresource_range
            .map(|range| (range.start, range.end - range.start))
            .unwrap_or((0, vk::WHOLE_SIZE));

        trace!("buffer_barrier {:?} {}..{}", buf, offset, offset + size);

        cmd::pipeline_barrier(
            &this.device,
            this.cmd_buf,
            None,
            &[BufferBarrier {
                previous_accesses: from_ref(&previous_access),
                next_accesses: from_ref(&next_access),
                src_queue_family_index: this.device.queue.family.idx,
                dst_queue_family_index: this.device.queue.family.idx,
                buffer: buf,
                offset: offset as _,
                size: size as _,
            }],
            &[],
        );
    }

    /// Signals that execution has completed and it is time to drop anything we collected.
    pub(crate) fn drop_fenced(this: &Self) {
        let mut droppables = this.droppables.borrow_mut();

        trace!("Dropping {} shared references", droppables.len());

        droppables.clear();
    }

    pub fn global_barrier(this: &Self, previous_access: AccessType, next_access: AccessType) {
        use std::slice::from_ref;

        trace!("global_barrier {:?} -> {:?}", previous_access, next_access,);

        cmd::pipeline_barrier(
            &this.device,
            this.cmd_buf,
            Some(GlobalBarrier {
                previous_accesses: from_ref(&previous_access),
                next_accesses: from_ref(&next_access),
            }),
            &[],
            &[],
        );
    }

    pub fn image_barrier(
        this: &Self,
        mut previous_access: AccessType,
        next_access: AccessType,
        image: vk::Image,
        subresource_range: Option<ImageSubresource>,
    ) {
        use std::slice::from_ref;

        fn layout(access: AccessType) -> ImageLayout {
            if matches!(access, AccessType::Present | AccessType::ComputeShaderWrite) {
                ImageLayout::General
            } else {
                ImageLayout::Optimal
            }
        }

        let previous_layout = layout(previous_access);
        let next_layout = layout(next_access);

        // // Preferring a global barrier if the layout does not need changes
        // if previous_access != AccessType::Nothing && previous_layout == next_layout {
        //     return Self::global_barrier(this, previous_access, next_access);
        // }

        trace!(
            "image_barrier {:?} {:?} -> {:?} (layout {:?} -> {:?})",
            image,
            previous_access,
            next_access,
            previous_layout,
            next_layout
        );

        cmd::pipeline_barrier(
            &this.device,
            this.cmd_buf,
            None,
            &[],
            &[ImageBarrier {
                previous_accesses: from_ref(&previous_access),
                next_accesses: from_ref(&next_access),
                previous_layout,
                next_layout,
                discard_contents: previous_access == AccessType::Nothing
                    || is_write_access(next_access),
                src_queue_family_index: this.device.queue.family.idx,
                dst_queue_family_index: this.device.queue.family.idx,
                image,
                range: subresource_range.unwrap_or_default().into_vk(),
            }],
        );
    }

    /// Drops an item after execution has been completed
    pub(crate) fn push_fenced_drop(this: &Self, thing_to_drop: impl Debug + 'static) {
        this.droppables.borrow_mut().push(Box::new(thing_to_drop));
    }

    pub fn queue_family_index(this: &Self) -> u32 {
        this.device.queue.family.idx
    }
}

impl<P> Deref for CommandBuffer<P>
where
    P: SharedPointerKind,
{
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.cmd_buf
    }
}

impl<P> Drop for CommandBuffer<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        use std::slice::from_ref;

        if panicking() {
            return;
        }

        unsafe {
            Device::wait_for_fence(&self.device, &self.fence).unwrap_or_default();

            self.device
                .free_command_buffers(self.pool, from_ref(&self.cmd_buf));
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_fence(self.fence, None);
        }
    }
}
