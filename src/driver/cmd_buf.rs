use {
    super::{Device, DriverError, QueueFamily},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    log::{trace, warn},
    std::{fmt::Debug, ops::Deref, thread::panicking},
};

#[derive(Debug)]
pub struct CommandBuffer<P>
where
    P: SharedPointerKind,
{
    cmd_buf: vk::CommandBuffer,
    pub(crate) device: SharedPointer<Device<P>, P>,
    droppables: Vec<Box<dyn Debug + Send + 'static>>,
    pub fence: vk::Fence, // Keeps state because everyone wants this
    pub pool: vk::CommandPool,
}

impl<P> CommandBuffer<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &SharedPointer<Device<P>, P>,
        queue_family: QueueFamily,
    ) -> Result<Self, DriverError> {
        let device = SharedPointer::clone(device);
        let cmd_pool_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::empty())
            .queue_family_index(queue_family.idx);
        let cmd_pool = unsafe {
            device
                .create_command_pool(&cmd_pool_info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        };
        let cmd_buf_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY);
        let cmd_buf = unsafe {
            device
                .allocate_command_buffers(&cmd_buf_info)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        }[0];
        let fence = unsafe {
            device
                .create_fence(
                    &vk::FenceCreateInfo::builder()
                        .flags(vk::FenceCreateFlags::SIGNALED)
                        .build(),
                    None,
                )
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        };

        Ok(Self {
            cmd_buf,
            device,
            droppables: vec![],
            fence,
            pool: cmd_pool,
        })
    }

    /// Signals that execution has completed and it is time to drop anything we collected.
    pub(crate) fn drop_fenced(this: &mut Self) {
        if !this.droppables.is_empty() {
            trace!("dropping {} shared references", this.droppables.len());
        }

        this.droppables.clear();
    }

    /// Drops an item after execution has been completed
    pub(crate) fn push_fenced_drop(this: &mut Self, thing_to_drop: impl Debug + Send + 'static) {
        this.droppables.push(Box::new(thing_to_drop));
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
            if Device::wait_for_fence(&self.device, &self.fence).is_err() {
                return;
            }

            self.device
                .free_command_buffers(self.pool, from_ref(&self.cmd_buf));
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_fence(self.fence, None);
        }
    }
}
