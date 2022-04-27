use {
    super::{
        driver::{
            image_access_layout, CommandBuffer, Device, DriverError, Swapchain, SwapchainError,
        },
        graph::{RenderGraph, SwapchainImageNode},
        ptr::Shared,
        HashPool,
    },
    archery::SharedPointerKind,
    ash::vk,
    log::trace,
    std::{error::Error, fmt::Formatter, time::Instant},
    vk_sync::{cmd::pipeline_barrier, AccessType, ImageBarrier, ImageLayout},
};

#[derive(Debug)]
pub struct Display<P>
where
    P: SharedPointerKind + Send,
{
    cache: HashPool<P>,
    cmd_bufs: Vec<[CommandBuffer<P>; 3]>,
    device: Shared<Device<P>, P>,
    swapchain: Swapchain<P>,
}

impl<P> Display<P>
where
    P: SharedPointerKind + Send + 'static,
{
    pub fn new(device: &Shared<Device<P>, P>, swapchain: Swapchain<P>) -> Self {
        let device = Shared::clone(device);

        Self {
            cache: HashPool::new(&device),
            cmd_bufs: Default::default(),
            device,
            swapchain,
        }
    }

    pub fn acquire_next_image(
        &mut self,
    ) -> Result<(SwapchainImageNode<P>, RenderGraph<P>), SwapchainError>
    where
        P: 'static,
    {
        trace!("acquire_next_image");

        let swapchain_image = self.swapchain.acquire_next_image()?;
        let mut render_graph = RenderGraph::new();
        let swapchain = render_graph.bind_node(swapchain_image);

        Ok((swapchain, render_graph))
    }

    unsafe fn begin(cmd_buf: &mut CommandBuffer<P>) -> Result<(), ()> {
        cmd_buf
            .device
            .reset_command_pool(cmd_buf.pool, vk::CommandPoolResetFlags::RELEASE_RESOURCES)
            .map_err(|_| ())?;
        cmd_buf
            .device
            .begin_command_buffer(
                **cmd_buf,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .map_err(|_| ())
    }

    pub fn present_image(
        &mut self,
        render_graph: RenderGraph<P>,
        swapchain_image: SwapchainImageNode<P>,
    ) -> Result<(), DisplayError> {
        use std::slice::from_ref;

        trace!("present_image");

        // The swapchain should have been written to, otherwise it would be noise and that's a panic
        let last_swapchain_access = render_graph
            .last_write(swapchain_image)
            .expect("uninitialized swapchain image: write something each frame!");
        let mut resolver = render_graph.resolve();
        let wait_dst_stage_mask = resolver.node_pipeline_stages(swapchain_image);
        let swapchain_node = swapchain_image;
        let swapchain_image = resolver.unbind_node(swapchain_node);
        let swapchain_image_idx = swapchain_image.idx as usize;

        while self.cmd_bufs.len() <= swapchain_image_idx {
            self.cmd_bufs.push([
                CommandBuffer::create(&self.device, self.device.queue.family)?,
                CommandBuffer::create(&self.device, self.device.queue.family)?,
                CommandBuffer::create(&self.device, self.device.queue.family)?,
            ]);
        }

        let cmd_bufs = &mut self.cmd_bufs[swapchain_image_idx];
        let cmd_buf = &mut cmd_bufs[0];

        let started = Instant::now();

        unsafe {
            Self::wait_for_fence(cmd_buf)?;
        }

        let mut wait_elapsed = Instant::now() - started;

        unsafe {
            Self::begin(cmd_buf)?;
        }

        resolver.record_node_dependencies(&mut self.cache, cmd_buf, swapchain_node)?;

        unsafe {
            // Record up to but not including the swapchain work
            Self::submit(
                cmd_buf,
                vk::SubmitInfo::builder().command_buffers(from_ref(cmd_buf)),
            )?;
        }

        // Switch commnd buffers because we're going to be submitting with a wait semaphore on the
        // swapchain image before we get access to record commands that use it
        let cmd_buf = &mut cmd_bufs[1];

        let wait_started = Instant::now();

        unsafe {
            Self::wait_for_fence(cmd_buf)?;
        }

        wait_elapsed += Instant::now() - wait_started;

        unsafe {
            Self::begin(cmd_buf)?;
        }

        resolver.record_node(&mut self.cache, cmd_buf, swapchain_node)?;

        pipeline_barrier(
            &cmd_buf.device,
            **cmd_buf,
            None,
            &[],
            from_ref(&ImageBarrier {
                previous_accesses: from_ref(&last_swapchain_access),
                next_accesses: from_ref(&AccessType::Present),
                previous_layout: image_access_layout(last_swapchain_access),
                next_layout: ImageLayout::General,
                discard_contents: false,
                src_queue_family_index: cmd_buf.device.queue.family.idx,
                dst_queue_family_index: cmd_buf.device.queue.family.idx,
                image: **swapchain_image,
                range: vk::ImageSubresourceRange {
                    layer_count: 1,
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_array_layer: 0,
                    base_mip_level: 0,
                    level_count: 1,
                },
            }),
        );

        unsafe {
            Self::submit(
                cmd_buf,
                vk::SubmitInfo::builder()
                    .command_buffers(from_ref(cmd_buf))
                    .signal_semaphores(from_ref(&swapchain_image.rendered))
                    .wait_semaphores(from_ref(&swapchain_image.acquired))
                    .wait_dst_stage_mask(from_ref(&wait_dst_stage_mask)),
            )?;
        }

        let cmd_buf = &mut cmd_bufs[2];

        let wait_started = Instant::now();

        unsafe {
            Self::wait_for_fence(cmd_buf)?;
        }

        wait_elapsed += Instant::now() - wait_started;

        // We may have unresolved nodes; things like copies that happen after present or operations
        // before present which use nodes that are unused in the remainder of the graph.
        // These operations are still important, but they don't need to wait for any of the above
        // things so we do them last
        if !resolver.is_resolved() {
            unsafe {
                Self::begin(cmd_buf)?;
            }

            resolver.record_unscheduled_passes(&mut self.cache, cmd_buf)?;

            unsafe {
                Self::submit(
                    cmd_buf,
                    vk::SubmitInfo::builder().command_buffers(from_ref(cmd_buf)),
                )
            }?;
        }

        let elapsed = Instant::now() - started - wait_elapsed;
        trace!(
            "command buffers: {} μs, delay: {} μs",
            elapsed.as_micros(),
            wait_elapsed.as_micros()
        );

        self.swapchain.present_image(swapchain_image);

        // Store the resolved graph because it contains bindings, leases, and other shared resources
        // that need to be kept alive until the fence is waited upon.
        CommandBuffer::push_fenced_drop(&mut self.cmd_bufs[swapchain_image_idx][2], resolver);

        Ok(())
    }

    unsafe fn submit(
        cmd_buf: &CommandBuffer<P>,
        submit_info: vk::SubmitInfoBuilder<'_>,
    ) -> Result<(), ()> {
        use std::slice::from_ref;

        cmd_buf
            .device
            .end_command_buffer(**cmd_buf)
            .map_err(|_| ())?;
        cmd_buf
            .device
            .reset_fences(from_ref(&cmd_buf.fence))
            .map_err(|_| ())?;
        cmd_buf
            .device
            .queue_submit(
                *cmd_buf.device.queue,
                from_ref(&*submit_info),
                cmd_buf.fence,
            )
            .map_err(|_| ())
    }

    unsafe fn wait_for_fence(cmd_buf: &mut CommandBuffer<P>) -> Result<(), ()> {
        Device::wait_for_fence(&cmd_buf.device, &cmd_buf.fence).map_err(|_| ())?;
        CommandBuffer::drop_fenced(cmd_buf);

        Ok(())
    }
}

#[derive(Debug)]
pub enum DisplayError {
    /// Unrecoverable device error; must destroy this device and display and start a new one
    DeviceLost,

    /// Recoverable driver error
    Driver(DriverError),
}

impl Error for DisplayError {}

impl From<()> for DisplayError {
    fn from(_: ()) -> Self {
        Self::DeviceLost
    }
}

impl From<DriverError> for DisplayError {
    fn from(err: DriverError) -> Self {
        Self::Driver(err)
    }
}

impl From<SwapchainError> for DisplayError {
    fn from(_: SwapchainError) -> Self {
        Self::DeviceLost
    }
}

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
