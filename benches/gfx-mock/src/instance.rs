use {super::*, raw_window_handle::HasRawWindowHandle};

#[derive(Debug)]
pub struct InstanceMock;

impl Instance<BackendMock> for InstanceMock {
    fn create(name: &str, version: u32) -> Result<Self, UnsupportedBackend> {
        Ok(InstanceMock)
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<BackendMock>> {
        let info = AdapterInfo {
            name: "Mock Device".to_string(),
            vendor: 0,
            device: 1234,
            device_type: DeviceType::Other,
        };
        let adapter = Adapter {
            info,
            physical_device: PhysicalDeviceMock,
            queue_families: vec![QueueFamilyMock],
        };

        vec![adapter]
    }

    unsafe fn create_surface(
        &self,
        raw_window_handle: &impl HasRawWindowHandle,
    ) -> Result<SurfaceMock, InitError> {
        let _handle = raw_window_handle.raw_window_handle();

        Ok(SurfaceMock)
    }

    unsafe fn destroy_surface(&self, _surface: SurfaceMock) {}
}
