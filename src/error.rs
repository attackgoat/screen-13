use gfx_hal::{
    device::{CreationError as DeviceCreationError, OutOfMemory as OutOfMemoryError},
    window::{AcquireError, CreationError as WindowCreationError, InitError},
    UnsupportedBackend,
};

use winit::error::OsError;

#[derive(Debug)]
pub enum Error {
    Gfx(GfxError),
    Os(OsError),
    WindowVideoModeNotFound,
}

impl Error {
    pub(crate) fn adapter() -> Self {
        Self::Gfx(GfxError::Backend(GfxBackendError::Adapter))
    }

    pub(crate) fn compute_queue_family() -> Self {
        Self::Gfx(GfxError::Backend(GfxBackendError::ComputeQueueFamily))
    }

    pub(crate) fn graphics_queue_family() -> Self {
        Self::Gfx(GfxError::Backend(GfxBackendError::GraphicsQueueFamily))
    }
}

impl From<AcquireError> for Error {
    fn from(error: AcquireError) -> Self {
        Self::Gfx(GfxError::Hal(GfxHalError::Acquire(error)))
    }
}

impl From<DeviceCreationError> for Error {
    fn from(error: DeviceCreationError) -> Self {
        Self::Gfx(GfxError::Hal(GfxHalError::DeviceCreation(error)))
    }
}

impl From<GfxError> for Error {
    fn from(error: GfxError) -> Self {
        Self::Gfx(error)
    }
}

impl From<InitError> for Error {
    fn from(error: InitError) -> Self {
        Self::Gfx(GfxError::Hal(GfxHalError::Init(error)))
    }
}

impl From<OutOfMemoryError> for Error {
    fn from(error: OutOfMemoryError) -> Self {
        Self::Gfx(GfxError::OutOfMemory(error))
    }
}

impl From<OsError> for Error {
    fn from(error: OsError) -> Self {
        Self::Os(error)
    }
}

impl From<UnsupportedBackend> for Error {
    fn from(error: UnsupportedBackend) -> Self {
        Self::Gfx(GfxError::Backend(GfxBackendError::Unsupported(error)))
    }
}

impl From<WindowCreationError> for Error {
    fn from(error: WindowCreationError) -> Self {
        Self::Gfx(GfxError::Hal(GfxHalError::WindowCreation(error)))
    }
}

#[derive(Debug)]
pub enum GfxBackendError {
    Adapter,
    ComputeQueueFamily,
    GraphicsQueueFamily,
    Unsupported(UnsupportedBackend),
}

#[derive(Debug)]
pub enum GfxError {
    Backend(GfxBackendError),
    Hal(GfxHalError),
    OutOfMemory(OutOfMemoryError),
}

/*impl From<GfxHalError> for GfxError {
    fn from(error: GfxHalError) -> Self {
        Self::Hal(error)
    }
}

impl From<UnsupportedBackend> for GfxError {
    fn from(error: UnsupportedBackend) -> Self {
        Self::Backend(error)
    }
}*/

#[derive(Debug)]
pub enum GfxHalError {
    Acquire(AcquireError),
    DeviceCreation(DeviceCreationError),
    Init(InitError),
    WindowCreation(WindowCreationError),
}

/*impl From<InitError> for GfxHalError {
    fn from(error: InitError) -> Self {
        Self::Init(error)
    }
}*/
