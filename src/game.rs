pub use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

use {
    crate::{
        gpu::{Driver, Gpu, Image, PhysicalDevice, Render, Swapchain, Texture, TextureRef},
        math::Extent,
    },
    gfx_hal::{queue::QueueType, window::Swapchain as _Swapchain},
    std::{cell::RefCell, cmp::Ordering},
    winit::monitor::VideoMode,
};

const MINIMUM_WINDOW_SIZE: usize = 420;

fn area(size: PhysicalSize<u32>) -> u32 {
    size.height * size.width
}

fn bit_depth_24(video_mode: &VideoMode) -> bool {
    video_mode.bit_depth() == 24
}

fn cmp_area_and_refresh_rate(lhs: &VideoMode, rhs: &VideoMode) -> Ordering {
    // Sort the video modes by area first ...
    match area(lhs.size()).cmp(&area(rhs.size())) {
        area @ Ordering::Greater | area @ Ordering::Less => return area,
        _ => (),
    }

    // ... and when they're equal sort by refresh rate
    lhs.refresh_rate().cmp(&rhs.refresh_rate())
}

/// The result of presenting a render to the screen. Hold this around for a few frames to
/// give the GPU time to finish processing it.
pub struct Frame(Render);

pub struct Game {
    back_buf: Vec<TextureRef<Image>>,
    event_loop: Option<EventLoop<()>>,
    dims: Extent,
    gpu: Gpu,
    swapchain: Swapchain,
    window: Window,
}

impl Game {
    pub fn fullscreen(title: &str, swapchain_len: usize) -> Self {
        let mut builder = WindowBuilder::new().with_title(title);
        let event_loop = EventLoop::new();
        let primary_monitor = event_loop.primary_monitor();
        // TODO: let dpi = primary_monitor.scale_factor();

        #[cfg(debug_assertions)]
        debug!("Building fullscreen window");

        // Sort all 24bit video modes from best to worst
        let mut video_modes = primary_monitor
            .video_modes()
            .filter(bit_depth_24)
            .collect::<Vec<_>>();
        video_modes.sort_by(cmp_area_and_refresh_rate);
        let best_video_mode = video_modes.pop().unwrap();
        let dims = best_video_mode.size().into();
        builder = builder.with_fullscreen(Some(Fullscreen::Exclusive(best_video_mode)));

        Self::new(event_loop, builder, dims, swapchain_len as u32)
    }

    fn new(
        event_loop: EventLoop<()>,
        builder: WindowBuilder,
        dims: Extent,
        swapchain_len: u32,
    ) -> Self {
        let window = builder.build(&event_loop).unwrap();
        let (gpu, surface) = Gpu::new(&window);
        let driver = Driver::clone(gpu.driver());
        let (swapchain, back_buf_images) = Swapchain::new(driver, surface, dims, swapchain_len);
        let back_buf = Vec::with_capacity(back_buf_images.len());

        Self {
            back_buf,
            dims,
            event_loop: Some(event_loop),
            gpu,
            swapchain,
            window,
        }
    }

    pub fn windowed(title: &str, swapchain_len: usize, dims: Extent) -> Self {
        let event_loop = EventLoop::new();
        let mut builder = WindowBuilder::new().with_title(title);

        // Setup fullscreen or windowed mode
        #[cfg(debug_assertions)]
        debug!("Building {}x{} window", dims.x, dims.y);
        let physical_dims: PhysicalSize<_> = dims.into();
        builder = builder
            .with_inner_size(physical_dims) // TODO: Rename
            .with_min_inner_size(LogicalSize::new(
                MINIMUM_WINDOW_SIZE as f32,
                MINIMUM_WINDOW_SIZE as f32,
            ));

        /*/ In windowed mode set the screen position
        if let Some((width, height)) = dimensions {
        let monitor = get_primary_monitor();
        let (monitor_width, monitor_height) = monitor.get_dimensions();
        let (half_monitor_width, half_monitor_height) = (monitor_width >> 1, monitor_height >> 1);
        let (half_window_width, half_window_height) = (width >> 1, height);
        let window_x = half_monitor_width.max(half_window_width) - half_window_width;
        let window_y = half_monitor_height.max(half_window_height) - half_window_height;

        window.set_position(window_x as i32, window_y as i32);
        }*/

        Self::new(event_loop, builder, dims, swapchain_len as u32)
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn present(&mut self, mut frame: Render) -> Option<Frame> {
        // Recreate the backbuffer textures if we have none
        if self.back_buf.is_empty() {
            self.recreate_swapchain();
        }

        let (idx, _suboptimal) = unsafe {
            // TODO: Handle suboptimal conditions
            match self.swapchain.acquire_image(0, None, None) {
                Err(_) => return None,
                Ok(image) => image,
            }
        };
        let texture = &mut self.back_buf[idx as usize];

        {
            let mut texture = texture.borrow_mut();
            unsafe {
                texture.acquire_swapchain();
            }
        }

        frame.present(
            #[cfg(debug_assertions)]
            "Game::present()",
            &texture,
        );

        let mut dropped = false;
        unsafe {
            self.swapchain
                .present_without_semaphores(
                    &mut self
                        .gpu
                        .driver()
                        .borrow_mut()
                        .queue_mut(QueueType::Graphics),
                    idx,
                )
                .unwrap_or_else(|_| {
                    dropped = true;
                    None
                });
        }

        if dropped {
            self.back_buf.clear();
        }

        Some(Frame(frame))
    }

    fn recreate_swapchain(&mut self) {
        self.back_buf.clear();
        for back_buf_image in Swapchain::recreate(&mut self.swapchain, self.dims) {
            self.back_buf
                .push(TextureRef::new(RefCell::new(Texture::from_swapchain(
                    back_buf_image,
                    self.gpu.driver(),
                    self.dims,
                    Swapchain::fmt(&self.swapchain),
                ))));
        }

        // TODO: ?
        //self.pool.borrow_mut().set_format(self.swapchain.format());
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn resize(&mut self, dims: Extent) {
        self.dims = dims;
        self.back_buf.clear();
    }

    pub fn run<F>(mut self, mut event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, ()>, &mut Game, &mut ControlFlow),
    {
        let event_loop = self.event_loop.take().unwrap();
        let mut game = self;

        event_loop.run(move |event, _, control_flow| event_handler(event, &mut game, control_flow));
    }
}
