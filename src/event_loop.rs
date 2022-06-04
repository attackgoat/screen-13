use {
    super::{
        display::{Display, DisplayError},
        driver::{Device, Driver, DriverConfigBuilder, DriverError},
        frame::FrameContext,
    },
    log::{debug, info, trace, warn},
    std::{
        mem::take,
        sync::Arc,
        time::{Duration, Instant},
    },
    winit::{
        event::{Event, WindowEvent},
        event_loop::ControlFlow,
        monitor::MonitorHandle,
        platform::run_return::EventLoopExtRunReturn,
        window::{Fullscreen, Window, WindowBuilder},
    },
};

pub fn run<FrameFn>(frame_fn: FrameFn) -> Result<(), DisplayError>
where
    FrameFn: FnMut(FrameContext),
{
    EventLoop::new().build()?.run(frame_fn)
}

pub enum FullscreenMode {
    Borderless,

    /// Seems to be the only way for stutter-free rendering on Nvidia + Win10.
    Exclusive,
}

// Pumps an operating system event loop in order to handle input and other events
// while drawing to the screen, continuously.
#[derive(Debug)]
pub struct EventLoop {
    pub device: Arc<Device>,
    display: Display,
    event_loop: winit::event_loop::EventLoop<()>,
    pub window: Window,
}

impl EventLoop {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> EventLoopBuilder {
        Default::default()
    }

    pub fn height(&self) -> u32 {
        self.window.inner_size().height
    }

    pub fn run<FrameFn>(mut self, mut frame_fn: FrameFn) -> Result<(), DisplayError>
    where
        FrameFn: FnMut(FrameContext),
    {
        let mut events = Vec::new();
        let mut will_exit = false;

        // Use the same delta-time smoothing as Kajiya; but start it off with a reasonable
        // guess so the following updates are even smoother
        const STANDARD_REFRESH_RATE: u16 = 60;
        let refresh_rate =
            self.window
                .fullscreen()
                .map(|mode| match mode {
                    Fullscreen::Exclusive(mode) => mode.refresh_rate(),
                    Fullscreen::Borderless(Some(monitor)) => monitor
                        .video_modes()
                        .next()
                        .map(|mode| mode.refresh_rate())
                        .unwrap_or(STANDARD_REFRESH_RATE),
                    _ => STANDARD_REFRESH_RATE,
                })
                .unwrap_or(STANDARD_REFRESH_RATE)
                .clamp(STANDARD_REFRESH_RATE, STANDARD_REFRESH_RATE << 2) as f32;
        let mut last_frame = Instant::now();
        let mut dt_filtered = 1.0 / refresh_rate;
        last_frame -= Duration::from_secs_f32(dt_filtered);

        debug!("first frame dt: {}", dt_filtered);

        while !will_exit {
            trace!("🟥🟩🟦 Event::RedrawRequested");

            self.event_loop.run_return(|event, _, control_flow| {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => {
                        *control_flow = ControlFlow::Exit;
                        will_exit = true;
                    }
                    Event::MainEventsCleared => *control_flow = ControlFlow::Exit,
                    _ => *control_flow = ControlFlow::Poll,
                }
                events.extend(event.to_static());
            });

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
                let dt_duration = now - last_frame;
                last_frame = now;

                let dt_raw = dt_duration.as_secs_f32();
                dt_filtered = dt_filtered + (dt_raw - dt_filtered) / 10.0;
            };

            let (swapchain, mut render_graph) = self.display.acquire_next_image()?;

            frame_fn(FrameContext {
                device: &self.device,
                dt: dt_filtered,
                height: self.height(),
                render_graph: &mut render_graph,
                events: take(&mut events).as_slice(),
                swapchain_image: swapchain,
                width: self.width(),
                window: &self.window,
                will_exit: &mut will_exit,
            });

            let elapsed = Instant::now() - now;

            trace!(
                "✅✅✅ render graph construction: {} μs ({}% load)",
                elapsed.as_micros(),
                ((elapsed.as_secs_f32() / refresh_rate) * 100.0) as usize,
            );

            self.display.present_image(render_graph, swapchain)?;
        }

        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.window.inner_size().width
    }

    pub fn window(&self) -> &Window {
        &self.window
    }
}

#[derive(Debug)]
pub struct EventLoopBuilder {
    driver_cfg: DriverConfigBuilder,
    event_loop: winit::event_loop::EventLoop<()>,
    window: WindowBuilder,
}

impl Default for EventLoopBuilder {
    fn default() -> Self {
        Self {
            driver_cfg: DriverConfigBuilder::default(),
            event_loop: winit::event_loop::EventLoop::new(),
            window: Default::default(),
        }
    }
}

impl EventLoopBuilder {
    /// Returns the list of all the monitors available on the system.
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        self.event_loop.available_monitors()
    }

    pub fn configure<ConfigureFn>(mut self, configure_fn: ConfigureFn) -> Self
    where
        ConfigureFn: FnOnce(DriverConfigBuilder) -> DriverConfigBuilder,
    {
        self.driver_cfg = configure_fn(self.driver_cfg);
        self
    }

    pub fn desired_swapchain_image_count(mut self, desired_swapchain_image_count: u32) -> Self {
        self.driver_cfg = self
            .driver_cfg
            .desired_swapchain_image_count(desired_swapchain_image_count);
        self
    }

    pub fn sync_display(mut self, sync_display: bool) -> Self {
        self.driver_cfg = self.driver_cfg.sync_display(sync_display);
        self
    }

    /// Sets up fullscreen mode using a conveience function. There are
    /// additional options offered by `winit` which can be accessed using
    /// the `with` function.
    pub fn fullscreen_mode(mut self, mode: FullscreenMode) -> Self {
        self.window = self.window.with_fullscreen(Some(match mode {
            FullscreenMode::Borderless => Fullscreen::Borderless(None),
            FullscreenMode::Exclusive => {
                if let Some(video_mode) = self
                    .event_loop
                    .primary_monitor()
                    .and_then(|monitor| monitor.video_modes().next())
                {
                    Fullscreen::Exclusive(video_mode)
                } else {
                    Fullscreen::Borderless(None)
                }
            }
        }));
        self
    }

    /// Enables Vulkan graphics debugging layers.
    ///
    /// _NOTE:_ Any valdation warnings or errors will cause the current thread to park itself after
    /// describing the error using the `log` crate. This makes it easy to attach a debugger and see
    /// what is causing the issue directly.
    pub fn debug(mut self, debug: bool) -> Self {
        self.driver_cfg = self.driver_cfg.debug(debug);
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

    pub fn ray_tracing(mut self, ray_tracing: bool) -> Self {
        self.driver_cfg = self.driver_cfg.ray_tracing(ray_tracing);
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
    pub fn window_mode(mut self) -> Self {
        self.window = self.window.with_fullscreen(None);
        self
    }
}

impl EventLoopBuilder {
    pub fn build(self) -> Result<EventLoop, DriverError> {
        let cfg = self
            .driver_cfg
            .build()
            .map_err(|_| DriverError::InvalidData)?; // TODO: More like invalid input

        // Create an operating system window via Winit
        let window = self.window.build(&self.event_loop).map_err(|err| {
            warn!("{err}");

            DriverError::Unsupported
        })?;
        let (width, height) = {
            let inner_size = window.inner_size();
            (inner_size.width, inner_size.height)
        };

        // Load the GPU driver (thin Vulkan device and swapchain smart pointers) and swapchain presenter/displayer
        let driver = Driver::new(&window, cfg, width, height)?;
        let display = Display::new(&driver.device, driver.swapchain);

        info!(
            "display resolution: {}x{} ({}x scale)",
            width,
            height,
            window.scale_factor() as f32,
        );

        Ok(EventLoop {
            device: Arc::clone(&driver.device),
            display,
            event_loop: self.event_loop,
            window,
        })
    }
}
