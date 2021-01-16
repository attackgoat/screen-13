use {super::{*, Backend}, gfx_hal::image::Usage};

#[derive(Debug)]
pub struct SurfaceMock;

impl Surface<Backend> for SurfaceMock {
    fn supports_queue_family(&self, _: &QueueFamilyMock) -> bool {
        true
    }

    fn capabilities(&self, _: &PhysicalDeviceMock) -> SurfaceCapabilities {
        let extents = {
            let min_extent = window::Extent2D {
                width: 0,
                height: 0,
            };
            let max_extent = window::Extent2D {
                width: 8192,
                height: 4096,
            };
            min_extent..=max_extent
        };
        let usage = Usage::COLOR_ATTACHMENT;
        let present_modes = PresentMode::all();
        let composite_alpha_modes = CompositeAlphaMode::OPAQUE;

        SurfaceCapabilities {
            image_count: 1..=1,
            current_extent: None,
            extents,
            max_image_layers: 1,
            usage,
            present_modes,
            composite_alpha_modes,
        }
    }

    fn supported_formats(&self, _: &PhysicalDeviceMock) -> Option<Vec<Format>> {
        None
    }
}

impl PresentationSurface<Backend> for SurfaceMock {
    type SwapchainImage = SwapchainImageMock;

    unsafe fn configure_swapchain(
        &mut self,
        _: &DeviceMock,
        _: SwapchainConfig,
    ) -> Result<(), SwapchainError> {
        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, _: &DeviceMock) {}

    unsafe fn acquire_image(
        &mut self,
        _: u64,
    ) -> Result<(SwapchainImageMock, Option<Suboptimal>), AcquireError> {
        Ok((SwapchainImageMock, None))
    }
}
