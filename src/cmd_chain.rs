use {
    super::{
        driver::{CommandBuffer, ComputePipeline, Device},
        ptr::SharedPointerKind,
        HashPool, Lease,
    },
    ash::vk,
    std::{
        error::Error,
        fmt::{Debug, Display, Formatter},
        thread::panicking,
    },
};

pub type CommandFn<P> = Box<dyn FnOnce(&Device<P>, &CommandBuffer<P>)>;

pub fn execute<C, P>(
    cmd_buf: C,
    func: impl FnOnce(&Device<P>, &CommandBuffer<P>) + 'static,
) -> CommandChain<C, P>
where
    C: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind,
{
    CommandChain {
        cmd_buf,
        funcs: vec![Box::new(func)],
    }
}

#[derive(Debug)]
pub struct ExecutionError;

impl Display for ExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ExecutionError {}

impl From<vk::Result> for ExecutionError {
    fn from(_: vk::Result) -> Self {
        Self
    }
}

pub struct CommandChain<C, P>
where
    C: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind,
{
    cmd_buf: C,
    funcs: Vec<CommandFn<P>>,
}

impl<C, P> CommandChain<C, P>
where
    C: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind,
{
    pub fn new(cmd_buf: C) -> Self {
        Self {
            cmd_buf,
            funcs: vec![],
        }
    }

    pub fn push_execute(
        mut self,
        func: impl FnOnce(&Device<P>, &CommandBuffer<P>) + 'static,
    ) -> Self {
        self.funcs.push(Box::new(func));
        self
    }

    /// Pushes "something" into a pile that won't be dropped until this command buffer submission
    /// actually finishes executing. Useful for keeping lifetimes in sync and handling undesired
    /// leases.
    pub fn push_shared_ref(mut self, shared_ref: impl Debug + 'static) -> Self {
        CommandBuffer::push_fenced_drop(self.cmd_buf.as_ref(), shared_ref);
        self
    }

    pub fn submit(mut self) -> Result<(), ExecutionError> {
        self.submit_mut()
    }

    fn submit_mut(&mut self) -> Result<(), ExecutionError> {
        use std::slice::from_ref;

        let cmd_buf = self.cmd_buf.as_ref();
        let device = &cmd_buf.device;

        unsafe {
            Device::wait_for_fence(device, &cmd_buf.fence).map_err(|_| ExecutionError)?;

            device
                .reset_command_pool(cmd_buf.pool, vk::CommandPoolResetFlags::RELEASE_RESOURCES)
                .map_err(|_| ExecutionError)?;
            device
                .begin_command_buffer(
                    **cmd_buf,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|_| ExecutionError)?;

            for func in self.funcs.drain(..) {
                func(device, cmd_buf);
            }

            device
                .end_command_buffer(**cmd_buf)
                .map_err(|_| ExecutionError)?;
            device
                .reset_fences(from_ref(&cmd_buf.fence))
                .map_err(|_| ExecutionError)?;
            device
                .queue_submit(
                    *device.queue,
                    from_ref(&vk::SubmitInfo::builder().command_buffers(from_ref(&*cmd_buf))),
                    cmd_buf.fence,
                )
                .map_err(|_| ExecutionError)?;
        }

        Ok(())
    }
}

impl<C, P> Drop for CommandChain<C, P>
where
    C: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        // Submit here if they did not ask while we were all happy and un-dropped
        if !self.funcs.is_empty() {
            self.submit_mut().unwrap(); // Call submit manually to handle errors
        }
    }
}

impl<C, P> From<C> for CommandChain<C, P>
where
    C: AsRef<CommandBuffer<P>>,
    P: SharedPointerKind,
{
    fn from(cmd_buf: C) -> Self {
        Self {
            cmd_buf,
            funcs: vec![],
        }
    }
}
