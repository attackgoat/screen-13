use {
    super::{Device, DriverError},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    gpu_allocator::{
        vulkan::{Allocation, AllocationCreateDesc},
        MemoryLocation,
    },
    log::trace,
    log::warn,
    std::{
        fmt::{Debug, Formatter},
        ops::{Deref, Range},
        thread::panicking,
    },
};

pub struct Buffer<P>
where
    P: SharedPointerKind,
{
    allocation: Option<Allocation>,
    buffer: vk::Buffer,
    device: SharedPointer<Device<P>, P>,
    pub info: BufferInfo,
    pub name: Option<String>,
}

impl<P> Buffer<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &SharedPointer<Device<P>, P>,
        info: impl Into<BufferInfo>,
    ) -> Result<Self, DriverError> {
        let info = info.into();

        trace!("create: {:?}", info);

        let device = SharedPointer::clone(device);
        let buffer_info = vk::BufferCreateInfo {
            size: info.size,
            usage: info.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let buffer = unsafe {
            device.create_buffer(&buffer_info, None).map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?
        };
        let mut requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        if info
            .usage
            .contains(vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR)
        {
            // TODO: query device props
            requirements.alignment = requirements.alignment.max(64);
        }

        let memory_location = if info.can_map {
            MemoryLocation::CpuToGpu
        } else {
            MemoryLocation::GpuOnly
        };
        let allocation = device
            .allocator
            .as_ref()
            .unwrap()
            .lock()
            .allocate(&AllocationCreateDesc {
                name: "buffer",
                requirements,
                location: memory_location,
                linear: true, // Buffers are always linear
            })
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;

        // Bind memory to the buffer
        unsafe {
            device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
        };

        Ok(Self {
            allocation: Some(allocation),
            buffer,
            device,
            info,
            name: None,
        })
    }

    pub fn copy_from_slice(this: &mut Self, offset: vk::DeviceSize, slice: &[u8]) {
        Self::mapped_slice_mut(this)[offset as _..offset as usize + slice.len()]
            .copy_from_slice(slice);
    }

    pub fn device_address(this: &Self) -> vk::DeviceAddress {
        unsafe {
            this.device.get_buffer_device_address(
                &vk::BufferDeviceAddressInfo::builder().buffer(this.buffer),
            )
        }
    }

    /// Returns a valid mapped pointer if the memory is host visible, otherwise it will panic.
    pub fn mapped_ptr<T>(this: &Self) -> *mut T {
        this.allocation
            .as_ref()
            .unwrap()
            .mapped_ptr()
            .unwrap()
            .as_ptr() as *mut _
    }

    /// Returns a valid mapped slice if the memory is host visible, otherwise it will panic.
    pub fn mapped_slice(this: &Self) -> &[u8] {
        &this.allocation.as_ref().unwrap().mapped_slice().unwrap()[0..this.info.size as usize]
    }

    /// Returns a valid mapped mutable slice if the memory is host visible, otherwise it will panic.
    pub fn mapped_slice_mut(this: &mut Self) -> &mut [u8] {
        &mut this
            .allocation
            .as_mut()
            .unwrap()
            .mapped_slice_mut()
            .unwrap()[0..this.info.size as usize]
    }
}

impl<P> Debug for Buffer<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{} ({:?})", name, self.buffer)
        } else {
            write!(f, "{:?}", self.buffer)
        }
    }
}

impl<P> Deref for Buffer<P>
where
    P: SharedPointerKind,
{
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<P> Drop for Buffer<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        self.device
            .allocator
            .as_ref()
            .unwrap()
            .lock()
            .free(self.allocation.take().unwrap())
            .unwrap_or_else(|_| warn!("Unable to free buffer allocation"));

        unsafe {
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Debug),
    pattern = "owned"
)]
pub struct BufferInfo {
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,

    /// Specifies a buffer whose memory is host visible.
    #[builder(default)]
    pub can_map: bool,
}

impl BufferInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        BufferInfoBuilder::default().size(size).usage(usage)
    }

    // TODO: This function is an opinon, should it be?
    pub fn new_mappable(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        Self::new(
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC,
        )
        .can_map(true)
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl BufferInfoBuilder {
    pub fn new(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> Self {
        Self::default().size(size).usage(usage)
    }

    pub fn build(self) -> BufferInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

impl From<BufferInfoBuilder> for BufferInfo {
    fn from(info: BufferInfoBuilder) -> Self {
        info.build()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferSubresource {
    pub start: vk::DeviceSize,
    pub end: vk::DeviceSize,
}

impl From<BufferInfo> for BufferSubresource {
    fn from(info: BufferInfo) -> Self {
        Self {
            start: 0,
            end: info.size,
        }
    }
}

impl From<Range<vk::DeviceSize>> for BufferSubresource {
    fn from(range: Range<vk::DeviceSize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<Option<Range<vk::DeviceSize>>> for BufferSubresource {
    fn from(range: Option<Range<vk::DeviceSize>>) -> Self {
        range.unwrap_or(0..vk::WHOLE_SIZE).into()
    }
}

impl From<BufferSubresource> for Range<vk::DeviceSize> {
    fn from(subresource: BufferSubresource) -> Self {
        subresource.start..subresource.end
    }
}
