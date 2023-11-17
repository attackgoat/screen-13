use {
    super::{
        display::{Display, DisplayError, ResolverPool},
        driver::{
            device::{Device, DeviceInfoBuilder},
            swapchain::{Swapchain, SwapchainInfoBuilder},
            DriverError, Surface,
        },
        frame::FrameContext,
        graph::RenderGraph,
        pool::hash::HashPool,
    },
    ash::vk,
    log::{debug, info, trace, warn},
    std::{
        fmt::{Debug, Formatter},
        mem::take,
        sync::Arc,
        time::{Duration, Instant},
    },
    winit::{
        event::{Event, WindowEvent},
        monitor::MonitorHandle,
        window::{Fullscreen, Window, WindowBuilder},
    },
};

/// Describes a screen mode for display.
pub enum FullscreenMode {
    /// A display mode which retains other operating system windows behind the current window.
    Borderless,

    /// Seems to be the only way for stutter-free rendering on Nvidia + Win10.
    Exclusive,
}

/// Pumps an operating system event loop in order to handle input and other events
/// while drawing to the screen, continuously.
#[derive(Debug)]
pub struct EventLoop {
    /// Provides access to the current graphics device.
    pub device: Arc<Device>,

    display: Display,
    event_loop: winit::event_loop::EventLoop<()>,
    swapchain: Swapchain,

    /// Provides access to the current operating system window.
    pub window: Window,
}

impl EventLoop {
    /// Specifies an event loop.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> EventLoopBuilder {
        Default::default()
    }

    /// Current window height, in pixels.
    pub fn height(&self) -> u32 {
        self.window.inner_size().height
    }

    /// Begins running a windowed event loop, providing `frame_fn` with a context of the current
    /// frame.
    pub fn run<FrameFn>(mut self, mut frame_fn: FrameFn) -> Result<(), DisplayError>
    where
        FrameFn: FnMut(FrameContext),
    {
        let mut events = Vec::new();
        let mut will_exit = false;

        // Use the same delta-time smoothing as Kajiya; but start it off with a reasonable
        // guess so the following updates are even smoother
        const STANDARD_REFRESH_RATE_MHZ: u32 = 60_000;
        let refresh_rate = (self
            .window
            .fullscreen()
            .map(|mode| match mode {
                Fullscreen::Exclusive(mode) => mode.refresh_rate_millihertz(),
                Fullscreen::Borderless(Some(monitor)) => monitor
                    .video_modes()
                    .next()
                    .map(|mode| mode.refresh_rate_millihertz())
                    .unwrap_or(STANDARD_REFRESH_RATE_MHZ),
                _ => STANDARD_REFRESH_RATE_MHZ,
            })
            .unwrap_or(STANDARD_REFRESH_RATE_MHZ)
            .clamp(STANDARD_REFRESH_RATE_MHZ, STANDARD_REFRESH_RATE_MHZ << 2)
            / 1_000) as f32;
        let mut last_frame = Instant::now();
        let mut dt_filtered = 1.0 / refresh_rate;
        last_frame -= Duration::from_secs_f32(dt_filtered);

        debug!("first frame dt: {}", dt_filtered);

        self.window.set_visible(true);

        self.event_loop
            .run(|event, window| {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => {
                        window.exit();
                    }
                    Event::WindowEvent {
                        event: WindowEvent::Focused(false),
                        ..
                    } => self.window.set_cursor_visible(true),
                    Event::AboutToWait => {
                        self.window.request_redraw();

                        return;
                    }
                    _ => (),
                }

                if !matches!(
                    event,
                    Event::WindowEvent {
                        event: WindowEvent::RedrawRequested,
                        ..
                    }
                ) {
                    events.push(event.clone());
                    return;
                }

                trace!("🟥🟩🟦 Event::RedrawRequested");
                profiling::scope!("Frame");

                if !events.is_empty() {
                    trace!("received {} events", events.len(),);
                }

                let now = Instant::now();

                // Filter the frame time before passing it to the application and renderer.
                // Fluctuations in frame rendering times cause stutter in animations,
                // and time-dependent effects (such as motion blur).
                //
                // Should applications need unfiltered delta time, they can calculate
                // it themselves, but it's good to pass the filtered time so users
                // don't need to worry about it.
                {
                    profiling::scope!("Calculate dt");

                    let dt_duration = now - last_frame;
                    last_frame = now;

                    let dt_raw = dt_duration.as_secs_f32();
                    dt_filtered = dt_filtered + (dt_raw - dt_filtered) / 10.0;
                };

                {
                    profiling::scope!("Update swapchain");

                    // Update the window size if it changes
                    let window_size = self.window.inner_size();
                    let mut swapchain_info = self.swapchain.info();
                    swapchain_info.width = window_size.width;
                    swapchain_info.height = window_size.height;
                    self.swapchain.set_info(swapchain_info);
                }

                let swapchain_image = self.swapchain.acquire_next_image();
                if swapchain_image.is_err() {
                    events.clear();

                    return;
                }

                let swapchain_image = swapchain_image.unwrap();
                let height = swapchain_image.info.height;
                let width = swapchain_image.info.width;
                let mut render_graph = RenderGraph::new();
                let swapchain_image = render_graph.bind_node(swapchain_image);

                {
                    profiling::scope!("Frame callback");

                    frame_fn(FrameContext {
                        device: &self.device,
                        dt: dt_filtered,
                        height,
                        render_graph: &mut render_graph,
                        events: take(&mut events).as_slice(),
                        swapchain_image,
                        width,
                        window: &self.window,
                        will_exit: &mut will_exit,
                    });
                }

                let elapsed = Instant::now() - now;

                trace!(
                    "✅✅✅ render graph construction: {} μs ({}% load)",
                    elapsed.as_micros(),
                    ((elapsed.as_secs_f32() / refresh_rate) * 100.0) as usize,
                );

                let swapchain_image = self
                    .display
                    .resolve_image(render_graph, swapchain_image)
                    .unwrap();
                self.window.pre_present_notify();
                self.swapchain.present_image(swapchain_image, 0, 0);

                profiling::finish_frame!();
            })
            .unwrap();

        self.window.set_visible(false);

        Ok(())
    }

    /// Current window width, in pixels.
    pub fn width(&self) -> u32 {
        self.window.inner_size().width
    }

    /// Current window.
    pub fn window(&self) -> &Window {
        &self.window
    }
}

impl AsRef<winit::event_loop::EventLoop<()>> for EventLoop {
    fn as_ref(&self) -> &winit::event_loop::EventLoop<()> {
        &self.event_loop
    }
}

/// Builder for `EventLoop`.
pub struct EventLoopBuilder {
    cmd_buf_count: usize,
    device_info: DeviceInfoBuilder,
    event_loop: winit::event_loop::EventLoop<()>,
    resolver_pool: Option<Box<dyn ResolverPool>>,
    surface_format_fn: Option<Box<dyn FnOnce(&[vk::SurfaceFormatKHR]) -> usize>>,
    swapchain_info: SwapchainInfoBuilder,
    window: WindowBuilder,
}

impl Debug for EventLoopBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventLoopBuilder")
    }
}

impl Default for EventLoopBuilder {
    fn default() -> Self {
        Self {
            cmd_buf_count: 5,
            device_info: DeviceInfoBuilder::default(),
            event_loop: winit::event_loop::EventLoop::new().unwrap(),
            resolver_pool: None,
            surface_format_fn: None,
            swapchain_info: SwapchainInfoBuilder::default(),
            window: Default::default(),
        }
    }
}

impl EventLoopBuilder {
    /// Returns the list of all the monitors available on the system.
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        self.event_loop.available_monitors()
    }

    /// Specifies the number of in-flight command buffers, which should be greater
    /// than or equal to the desired swapchain image count.
    ///
    /// More command buffers mean less time waiting for previously submitted frames to complete, but
    /// more memory in use.
    ///
    /// Generally a value of one or two greater than desired image count produces the smoothest
    /// animation.
    pub fn command_buffer_count(mut self, cmd_buf_count: usize) -> Self {
        self.cmd_buf_count = cmd_buf_count;
        self
    }

    /// A function to select the desired swapchain surface image format.
    pub fn desired_surface_format<F>(mut self, surface_format_fn: F) -> Self
    where
        F: FnOnce(&[vk::SurfaceFormatKHR]) -> usize + 'static,
    {
        let surface_format_fn = Box::new(surface_format_fn);
        self.surface_format_fn = Some(surface_format_fn);
        self
    }

    /// The desired, but not guaranteed, number of images that will be in the created swapchain.
    ///
    /// More images introduces more display lag, but smoother animation.
    pub fn desired_swapchain_image_count(mut self, desired_swapchain_image_count: u32) -> Self {
        self.swapchain_info = self
            .swapchain_info
            .desired_image_count(desired_swapchain_image_count);
        self
    }

    /// Set to `true` to enable vsync in exclusive fullscreen video modes.
    pub fn sync_display(mut self, sync_display: bool) -> Self {
        self.swapchain_info = self.swapchain_info.sync_display(sync_display);
        self
    }

    /// Sets up fullscreen mode. In addition, decorations are set to `false` and maximized is set to
    /// `true`.
    ///
    /// # Note
    ///
    /// There are additional options offered by `winit` which can be accessed using the `window`
    /// function.
    pub fn fullscreen_mode(mut self, mode: FullscreenMode) -> Self {
        let inner_size;
        self.window = self
            .window
            .with_decorations(false)
            .with_maximized(true)
            .with_fullscreen(Some(match mode {
                FullscreenMode::Borderless => {
                    info!("Using borderless fullscreen");

                    inner_size = None;

                    Fullscreen::Borderless(None)
                }
                FullscreenMode::Exclusive => {
                    if let Some(video_mode) =
                        self.event_loop.primary_monitor().and_then(|monitor| {
                            let monitor_size = monitor.size();
                            monitor.video_modes().find(|mode| {
                                let mode_size = mode.size();

                                // Don't pick a mode which has greater resolution than the monitor is
                                // currently using: it causes a panic on x11 in winit
                                mode_size.height <= monitor_size.height
                                    && mode_size.width <= monitor_size.width
                            })
                        })
                    {
                        info!(
                            "Using {}x{} {}bpp @ {}hz exclusive fullscreen",
                            video_mode.size().width,
                            video_mode.size().height,
                            video_mode.bit_depth(),
                            video_mode.refresh_rate_millihertz() / 1_000
                        );

                        inner_size = Some(video_mode.size());

                        Fullscreen::Exclusive(video_mode)
                    } else {
                        warn!("Using borderless fullscreen");

                        inner_size = None;

                        Fullscreen::Borderless(None)
                    }
                }
            }));

        if let Some(inner_size) = inner_size.or_else(|| {
            self.event_loop
                .primary_monitor()
                .map(|monitor| monitor.size())
        }) {
            self.window = self.window.with_inner_size(inner_size);
        }

        self
    }

    /// Enables Vulkan graphics debugging layers.
    ///
    /// _NOTE:_ Any valdation warnings or errors will cause the current thread to park itself after
    /// describing the error using the `log` crate. This makes it easy to attach a debugger and see
    /// what is causing the issue directly.
    ///
    /// ## Platform-specific
    ///
    /// **macOS:** Has no effect.
    pub fn debug(mut self, debug: bool) -> Self {
        self.device_info = self.device_info.debug(debug);
        self
    }

    /// Returns the primary monitor of the system.
    ///
    /// Returns `None` if it can't identify any monitor as a primary one.
    ///
    /// ## Platform-specific
    ///
    /// **Wayland:** Always returns `None`.
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.event_loop.primary_monitor()
    }

    /// Allows for specification of a custom pool implementation.
    ///
    /// This pool will hold leases for Vulkan objects needed by [`Display`].
    pub fn resolver_pool(mut self, pool: Box<dyn ResolverPool>) -> Self {
        self.resolver_pool = Some(pool);
        self
    }

    /// Allows deeper customization of the window, if needed.
    pub fn window<WindowFn>(mut self, window_fn: WindowFn) -> Self
    where
        WindowFn: FnOnce(WindowBuilder) -> WindowBuilder,
    {
        self.window = window_fn(self.window);
        self
    }

    /// Sets up "windowed" mode, which is the opposite of fullscreen.
    ///
    /// # Note
    ///
    /// There are additional options offered by `winit` which can be accessed using the `window`
    /// function.
    pub fn window_mode(mut self) -> Self {
        self.window = self.window.with_fullscreen(None);
        self
    }
}

impl EventLoopBuilder {
    /// Builds a new `EventLoop`.
    pub fn build(self) -> Result<EventLoop, DriverError> {
        // Create an operating system window via Winit
        let window = self.window;

        #[cfg(not(target_os = "macos"))]
        let window = window.with_visible(false);

        let window = window.build(&self.event_loop).map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;
        let (width, height) = {
            let inner_size = window.inner_size();
            (inner_size.width, inner_size.height)
        };

        // Load the GPU driver (thin Vulkan device and swapchain smart pointers)
        let device_info = self.device_info.build();
        let device = Arc::new(Device::create_display_window(device_info, &window)?);

        // TODO: Select a better index
        let queue_family_index = 0;

        // Create a display that is cached using the given pool implementation
        let pool = self
            .resolver_pool
            .unwrap_or_else(|| Box::new(HashPool::new(&device)));
        let display = Display::new(&device, pool, self.cmd_buf_count, queue_family_index)?;

        let surface = Surface::new(&device, &window)?;
        let surface_formats = Surface::formats(&surface)?;

        if surface_formats.is_empty() {
            warn!("invalid surface formats");

            return Err(DriverError::Unsupported);
        }

        for surface in &surface_formats {
            debug!(
                "surface: {:#?} ({:#?})",
                surface.format, surface.color_space
            );
        }

        let surface_format_fn = self
            .surface_format_fn
            .unwrap_or_else(|| Box::new(Self::select_swapchain_format));
        let mut surface_format_idx = surface_format_fn(&surface_formats);

        if surface_format_idx >= surface_formats.len() {
            warn!("invalid surface format selected");

            surface_format_idx = 0;
        }

        let surface_format = surface_formats[surface_format_idx];
        let swapchain = Swapchain::new(
            &device,
            surface,
            self.swapchain_info.format(surface_format).build(),
        )?;

        info!(
            "Window dimensions: {}x{} ({}x scale)",
            width,
            height,
            window.scale_factor() as f32,
        );

        Ok(EventLoop {
            device,
            display,
            event_loop: self.event_loop,
            swapchain,
            window,
        })
    }

    fn select_swapchain_format(formats: &[vk::SurfaceFormatKHR]) -> usize {
        for (idx, format) in formats.iter().copied().enumerate() {
            if format.color_space != vk::ColorSpaceKHR::SRGB_NONLINEAR {
                continue;
            }

            if matches!(
                format.format,
                vk::Format::R8G8B8A8_SRGB | vk::Format::B8G8R8A8_SRGB
            ) {
                return idx;
            }
        }

        for (idx, format) in formats.iter().copied().enumerate() {
            if matches!(
                format.format,
                vk::Format::R8G8B8A8_UNORM | vk::Format::B8G8R8A8_UNORM
            ) {
                return idx;
            }
        }

        0
    }
}
