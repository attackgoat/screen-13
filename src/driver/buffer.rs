//! Buffer resource types

use {
    super::{access_type_from_u8, access_type_into_u8, Device, DriverError},
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    gpu_allocator::{
        vulkan::{Allocation, AllocationCreateDesc},
        MemoryLocation,
    },
    log::trace,
    log::warn,
    std::{
        fmt::{Debug, Formatter},
        ops::{Deref, Range},
        sync::{
            atomic::{AtomicU8, Ordering},
            Arc,
        },
        thread::panicking,
    },
    vk_sync::AccessType,
};

/// Smart pointer handle to a [buffer] object.
///
/// Also contains information about the object.
///
/// ## `Deref` behavior
///
/// `Buffer` automatically dereferences to [`vk::Buffer`] (via the [`Deref`][deref] trait), so you
/// can call `vk::Buffer`'s methods on a value of type `Buffer`. To avoid name clashes with
/// `vk::Buffer`'s methods, the methods of `Buffer` itself are associated functions, called using
/// [fully qualified syntax]:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{AccessType, Device, DriverConfig, DriverError};
/// # use screen_13::driver::buffer::{Buffer, BufferInfo};
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
/// # let info = BufferInfo::new(8, vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);
/// # let my_buf = Buffer::create(&device, info)?;
/// let addr = Buffer::device_address(&my_buf);
/// # Ok(()) }
/// ```
///
/// [buffer]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkBuffer.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
pub struct Buffer {
    allocation: Option<Allocation>,
    buffer: vk::Buffer,
    device: Arc<Device>,

    /// Information used to create this object.
    pub info: BufferInfo,

    /// A name for debugging purposes.
    pub name: Option<String>,

    prev_access: AtomicU8,
}

impl Buffer {
    /// Creates a new buffer on the given device.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// const SIZE: vk::DeviceSize = 1024;
    /// let info = BufferInfo::new_mappable(SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER);
    /// let buf = Buffer::create(&device, info)?;
    ///
    /// assert_ne!(*buf, vk::Buffer::null());
    /// assert_eq!(buf.info.size, SIZE);
    /// # Ok(()) }
    /// ```
    pub fn create(device: &Arc<Device>, info: impl Into<BufferInfo>) -> Result<Self, DriverError> {
        let info = info.into();

        trace!("create: {:?}", info);

        let device = Arc::clone(device);
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
            prev_access: AtomicU8::new(access_type_into_u8(AccessType::Nothing)),
        })
    }

    /// Creates a new mappable buffer on the given device and fills it with the data in `slice`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// const DATA: [u8; 4] = [0xfe, 0xed, 0xbe, 0xef];
    /// let buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::UNIFORM_BUFFER, &DATA)?;
    ///
    /// assert_ne!(*buf, vk::Buffer::null());
    /// assert_eq!(buf.info.size, 4);
    /// assert_eq!(Buffer::mapped_slice(&buf), &DATA);
    /// # Ok(()) }
    /// ```
    pub fn create_from_slice(
        device: &Arc<Device>,
        usage: vk::BufferUsageFlags,
        slice: &[u8],
    ) -> Result<Self, DriverError> {
        let info = BufferInfo {
            can_map: true,
            size: slice.len() as vk::DeviceSize,
            usage,
        };
        let mut buffer = Self::create(device, info)?;

        Self::copy_from_slice(&mut buffer, 0, slice);

        Ok(buffer)
    }

    /// Keeps track of some `next_access` which affects this object.
    ///
    /// Returns the previous access for which a pipeline barrier should be used to prevent data
    /// corruption.
    ///
    /// # Note
    ///
    /// Used to maintain object state when passing a _Screen 13_-created `vk::Buffer` handle to
    /// external code such as [_Ash_] or [_Erupt_] bindings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{AccessType, Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # const SIZE: vk::DeviceSize = 1024;
    /// # let info = BufferInfo::new(SIZE, vk::BufferUsageFlags::STORAGE_BUFFER);
    /// # let my_buf = Buffer::create(&device, info)?;
    /// // Initially we want to "Read Other"
    /// let next = AccessType::ComputeShaderReadOther;
    /// let prev = Buffer::access(&my_buf, next);
    /// assert_eq!(prev, AccessType::Nothing);
    ///
    /// // External code may now "Read Other"; no barrier required
    ///
    /// // Subsequently we want to "Write"
    /// let next = AccessType::ComputeShaderWrite;
    /// let prev = Buffer::access(&my_buf, next);
    /// assert_eq!(prev, AccessType::ComputeShaderReadOther);
    ///
    /// // A barrier on "Read Other" before "Write" is required!
    /// # Ok(()) }
    /// ```
    ///
    /// [_Ash_]: https://crates.io/crates/ash
    /// [_Erupt_]: https://crates.io/crates/erupt
    pub fn access(this: &Self, next_access: AccessType) -> AccessType {
        access_type_from_u8(
            this.prev_access
                .swap(access_type_into_u8(next_access), Ordering::Relaxed),
        )
    }

    /// Updates a mappable buffer starting at `offset` with the data in `slice`.
    ///
    /// # Panics
    ///
    /// Panics if the buffer was not created with the `can_map` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let info = BufferInfo::new_mappable(4, vk::BufferUsageFlags::empty());
    /// # let mut my_buf = Buffer::create(&device, info)?;
    /// const DATA: [u8; 4] = [0xde, 0xad, 0xc0, 0xde];
    /// Buffer::copy_from_slice(&mut my_buf, 0, &DATA);
    ///
    /// assert_eq!(Buffer::mapped_slice(&my_buf), &DATA);
    /// # Ok(()) }
    /// ```
    pub fn copy_from_slice(this: &mut Self, offset: vk::DeviceSize, slice: &[u8]) {
        Self::mapped_slice_mut(this)[offset as _..offset as usize + slice.len()]
            .copy_from_slice(slice);
    }

    /// Returns the device address of this object.
    ///
    /// # Panics
    ///
    /// Panics if the buffer was not created with the `SHADER_DEVICE_ADDRESS` usage flag.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # let info = BufferInfo::new_mappable(4, vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);
    /// # let my_buf = Buffer::create(&device, info)?;
    /// let addr = Buffer::device_address(&my_buf);
    ///
    /// assert_ne!(addr, 0);
    /// # Ok(()) }
    /// ```
    pub fn device_address(this: &Self) -> vk::DeviceAddress {
        unsafe {
            this.device.get_buffer_device_address(
                &vk::BufferDeviceAddressInfo::builder().buffer(this.buffer),
            )
        }
    }

    /// Returns a mapped slice.
    ///
    /// # Panics
    ///
    /// Panics if the buffer was not created with the `can_map` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # const DATA: [u8; 4] = [0; 4];
    /// # let my_buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::empty(), &DATA)?;
    /// // my_buf is mappable and filled with four zeroes
    /// let data = Buffer::mapped_slice(&my_buf);
    ///
    /// assert_eq!(data.len(), 4);
    /// assert_eq!(data[0], 0x00);
    /// # Ok(()) }
    /// ```
    pub fn mapped_slice(this: &Self) -> &[u8] {
        &this.allocation.as_ref().unwrap().mapped_slice().unwrap()[0..this.info.size as usize]
    }

    /// Returns a mapped mutable slice.
    ///
    /// # Panics
    ///
    /// Panics if the buffer was not created with the `can_map` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use glam::Mat4;
    /// # use screen_13::driver::{Device, DriverConfig, DriverError};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::new(DriverConfig::new().build())?);
    /// # const DATA: [u8; 4] = [0; 4];
    /// # let mut my_buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::empty(), &DATA)?;
    /// let mut data = Buffer::mapped_slice_mut(&mut my_buf);
    /// data.copy_from_slice(&42f32.to_be_bytes());
    ///
    /// assert_eq!(data.len(), 4);
    /// assert_eq!(data[0], 0x42);
    /// # Ok(()) }
    /// ```
    pub fn mapped_slice_mut(this: &mut Self) -> &mut [u8] {
        &mut this
            .allocation
            .as_mut()
            .unwrap()
            .mapped_slice_mut()
            .unwrap()[0..this.info.size as usize]
    }
}

impl Debug for Buffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{} ({:?})", name, self.buffer)
        } else {
            write!(f, "{:?}", self.buffer)
        }
    }
}

impl Deref for Buffer {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl Drop for Buffer {
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

/// Information used to create a [`Buffer`] instance.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build", error = "BufferInfoBuilderError"),
    derive(Debug),
    pattern = "owned"
)]
pub struct BufferInfo {
    /// Size in bytes of the buffer to be created.
    #[builder(default)]
    pub size: vk::DeviceSize,

    /// A bitmask of specifying allowed usages of the buffer.
    #[builder(default)]
    pub usage: vk::BufferUsageFlags,

    /// Specifies a buffer whose memory is host visible and may be mapped.
    #[builder(default)]
    pub can_map: bool,
}

impl BufferInfo {
    /// Specifies a non-mappable buffer with the given `size` and `usage` values.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        BufferInfoBuilder::default().size(size).usage(usage)
    }

    /// Specifies a mappable buffer with the given `size` and `usage` values.
    ///
    /// # Note
    ///
    /// For conveience the given usage value will be bitwise OR'd with
    /// `TRANSFER_DST | TRANSFER_SRC`.
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
    /// Builds a new `BufferInfo`.
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

#[derive(Debug)]
struct BufferInfoBuilderError;

impl From<UninitializedFieldError> for BufferInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Specifies a range of buffer data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferSubresource {
    /// The start of range.
    pub start: vk::DeviceSize,

    /// The non-inclusive end of the range.
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
