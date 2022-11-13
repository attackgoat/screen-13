use {
    super::{
        driver::{
            image_access_layout, CommandBuffer, CommandBufferInfo, Device, DriverError,
            SwapchainImage,
        },
        graph::{node::SwapchainImageNode, RenderGraph, ResolverPool},
    },
    ash::vk,
    log::trace,
    std::{
        error::Error,
        fmt::{Debug, Formatter},
        sync::Arc,
        time::Instant,
    },
    vk_sync::{cmd::pipeline_barrier, AccessType, ImageBarrier, ImageLayout},
};

/// A physical display interface.
pub struct Display {
    cmd_buf_idx: usize,
    cmd_bufs: Vec<CommandBuffer>,
    pool: Box<dyn ResolverPool>,
}

impl Display {
    /// Constructs a new `Display` object.
    pub fn new(device: &Arc<Device>, pool: Box<dyn ResolverPool>, cmd_buf_count: usize) -> Self {
        let mut cmd_bufs = Vec::with_capacity(cmd_buf_count);
        for _ in 0..cmd_buf_count {
            cmd_bufs.push(CommandBuffer::create(device, CommandBufferInfo).unwrap());
        }

        Self {
            cmd_buf_idx: 0,
            cmd_bufs,
            pool,
        }
    }

    unsafe fn begin(cmd_buf: &mut CommandBuffer) -> Result<(), ()> {
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

    /// Displays the given swapchain image using passes specified in `render_graph`, if possible.
    pub fn resolve_image(
        &mut self,
        render_graph: RenderGraph,
        swapchain_image: SwapchainImageNode,
    ) -> Result<SwapchainImage, DisplayError> {
        use std::slice::from_ref;

        trace!("present_image");

        // The swapchain should have been written to, otherwise it would be noise and that's a panic
        let last_swapchain_access = render_graph
            .last_write(swapchain_image)
            .expect("uninitialized swapchain image: write something each frame!");
        let mut resolver = render_graph.resolve();
        let wait_dst_stage_mask = resolver.node_pipeline_stages(swapchain_image);

        self.cmd_buf_idx += 1;
        self.cmd_buf_idx %= self.cmd_bufs.len();

        let cmd_buf = &mut self.cmd_bufs[self.cmd_buf_idx];

        let started = Instant::now();

        unsafe {
            Self::wait_for_fence(cmd_buf)?;
        }

        unsafe {
            Self::begin(cmd_buf)?;
        }

        // resolver.record_node_dependencies(&mut *self.pool, cmd_buf, swapchain_image)?;
        resolver.record_node(&mut *self.pool, cmd_buf, swapchain_image)?;

        let swapchain_image = resolver.unbind_node(swapchain_image);

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
                src_queue_family_index: cmd_buf.device.queues[0].family.idx,
                dst_queue_family_index: cmd_buf.device.queues[0].family.idx,
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

        // We may have unresolved nodes; things like copies that happen after present or operations
        // before present which use nodes that are unused in the remainder of the graph.
        // These operations are still important, but they don't need to wait for any of the above
        // things so we do them last
        resolver.record_unscheduled_passes(&mut *self.pool, cmd_buf)?;

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

        let elapsed = Instant::now() - started;
        trace!("🔜🔜🔜 vkQueueSubmit took {} μs", elapsed.as_micros(),);

        // Store the resolved graph because it contains bindings, leases, and other shared resources
        // that need to be kept alive until the fence is waited upon.
        CommandBuffer::push_fenced_drop(cmd_buf, resolver);

        Ok(swapchain_image)
    }

    unsafe fn submit(
        cmd_buf: &CommandBuffer,
        submit_info: vk::SubmitInfoBuilder<'_>,
    ) -> Result<(), ()> {
        use std::slice::from_ref;

        cmd_buf
            .device
            .end_command_buffer(**cmd_buf)
            .map_err(|_| ())?;
        cmd_buf
            .device
            .queue_submit(
                *cmd_buf.device.queues[0],
                from_ref(&*submit_info),
                cmd_buf.fence,
            )
            .map_err(|_| ())
    }

    unsafe fn wait_for_fence(cmd_buf: &mut CommandBuffer) -> Result<(), ()> {
        use std::slice::from_ref;

        Device::wait_for_fence(&cmd_buf.device, &cmd_buf.fence).map_err(|_| ())?;
        CommandBuffer::drop_fenced(cmd_buf);

        cmd_buf
            .device
            .reset_fences(from_ref(&cmd_buf.fence))
            .map_err(|_| ())
    }
}

impl Debug for Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Display")
    }
}

/// Describes error conditions relating to physical displays.
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

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
