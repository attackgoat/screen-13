use {
    super::{Device, DriverError},
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    gpu_allocator::{
        vulkan::{Allocation, AllocationCreateDesc},
        MemoryLocation,
    },
    log::trace,
    log::warn,
    std::{
        ops::{Deref, Range},
        thread::panicking,
    },
};

#[derive(Debug)]
pub struct Buffer<P>
where
    P: SharedPointerKind,
{
    allocation: Option<Allocation>,
    buffer: vk::Buffer,
    device: Shared<Device<P>, P>,
    pub info: BufferInfo,
}

impl<P> Buffer<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &Shared<Device<P>, P>,
        info: impl Into<BufferInfo>,
    ) -> Result<Self, DriverError> {
        trace!("create");

        let info = info.into();
        let device = Shared::clone(device);
        let buffer_info = vk::BufferCreateInfo {
            size: info.size as u64,
            usage: info.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let buffer = unsafe {
            device
                .create_buffer(&buffer_info, None)
                .map_err(|_| DriverError::Unsupported)?
        };
        let mut requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        // TODO: why does `get_buffer_memory_requirements` fail to get the correct alignment on AMD?
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
            .map_err(|_| DriverError::Unsupported)?;

        // Bind memory to the buffer
        unsafe {
            device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|_| DriverError::Unsupported)?
        };

        Ok(Self {
            allocation: Some(allocation),
            buffer,
            device,
            info,
        })
    }

    pub fn device_address(this: &Self) -> u64 {
        unsafe {
            this.device.get_buffer_device_address(
                &ash::vk::BufferDeviceAddressInfo::builder().buffer(this.buffer),
            )
        }
    }

    pub fn mapped_slice_mut(this: &mut Self) -> &mut [u8] {
        &mut this
            .allocation
            .as_mut()
            .unwrap()
            .mapped_slice_mut()
            .unwrap()[0..this.info.size as usize]
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
#[builder(pattern = "owned")]
pub struct BufferInfo {
    pub size: u64,
    pub usage: vk::BufferUsageFlags,
    #[builder(default)]
    pub can_map: bool,
}

impl BufferInfo {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(size: u64, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        BufferInfoBuilder::default().size(size).usage(usage)
    }
}

impl From<BufferInfoBuilder> for BufferInfo {
    fn from(info: BufferInfoBuilder) -> Self {
        info.build().unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferSubresource {
    pub range_start: u64,
    pub range_end: u64,
}

impl From<BufferInfo> for BufferSubresource {
    fn from(info: BufferInfo) -> Self {
        Self {
            range_start: 0,
            range_end: info.size as u64,
        }
    }
}

impl From<Range<u64>> for BufferSubresource {
    fn from(range: Range<u64>) -> Self {
        Self {
            range_start: range.start,
            range_end: range.end,
        }
    }
}

impl From<Option<Range<u64>>> for BufferSubresource {
    fn from(range: Option<Range<u64>>) -> Self {
        range.unwrap_or(0..vk::WHOLE_SIZE).into()
    }
}

impl From<BufferSubresource> for Range<u64> {
    fn from(subresource: BufferSubresource) -> Self {
        subresource.range_start..subresource.range_end
    }
}
