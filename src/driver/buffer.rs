//! Buffer resource types

use {
    super::{access_type_from_u8, access_type_into_u8, device::Device, DriverError},
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    gpu_allocator::{
        vulkan::{Allocation, AllocationCreateDesc, AllocationScheme},
        MemoryLocation,
    },
    log::trace,
    log::warn,
    std::{
        fmt::{Debug, Formatter},
        mem::ManuallyDrop,
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
/// `Buffer` automatically dereferences to [`vk::Buffer`] (via the [`Deref`] trait), so you
/// can call `vk::Buffer`'s methods on a value of type `Buffer`. To avoid name clashes with
/// `vk::Buffer`'s methods, the methods of `Buffer` itself are associated functions, called using
/// [fully qualified syntax]:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{AccessType, DriverError};
/// # use screen_13::driver::device::{Device, DeviceInfo};
/// # use screen_13::driver::buffer::{Buffer, BufferInfo};
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
/// # let info = BufferInfo::device_mem(8, vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);
/// # let my_buf = Buffer::create(&device, info)?;
/// let addr = Buffer::device_address(&my_buf);
/// # Ok(()) }
/// ```
///
/// [buffer]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkBuffer.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
pub struct Buffer {
    allocation: ManuallyDrop<Allocation>,
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
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// const SIZE: vk::DeviceSize = 1024;
    /// let info = BufferInfo::host_mem(SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER);
    /// let buf = Buffer::create(&device, info)?;
    ///
    /// assert_ne!(*buf, vk::Buffer::null());
    /// assert_eq!(buf.info.size, SIZE);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn create(device: &Arc<Device>, info: impl Into<BufferInfo>) -> Result<Self, DriverError> {
        let info = info.into();

        trace!("create: {:?}", info);

        debug_assert_ne!(info.size, 0, "Size must be non-zero");

        let device = Arc::clone(device);
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(info.size)
            .usage(info.usage)
            .sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&device.physical_device.queue_family_indices);
        let buffer = unsafe {
            device.create_buffer(&buffer_info, None).map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?
        };
        let mut requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        requirements.alignment = requirements.alignment.max(info.alignment);

        let memory_location = if info.mappable {
            MemoryLocation::CpuToGpu
        } else {
            MemoryLocation::GpuOnly
        };
        let allocation = {
            profiling::scope!("allocate");

            #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
            let mut allocator = device.allocator.lock();

            #[cfg(not(feature = "parking_lot"))]
            let mut allocator = allocator.unwrap();

            allocator
                .allocate(&AllocationCreateDesc {
                    name: "buffer",
                    requirements,
                    location: memory_location,
                    linear: true, // Buffers are always linear
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })
        }?;

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
            allocation: ManuallyDrop::new(allocation),
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
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// const DATA: [u8; 4] = [0xfe, 0xed, 0xbe, 0xef];
    /// let buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::UNIFORM_BUFFER, &DATA)?;
    ///
    /// assert_ne!(*buf, vk::Buffer::null());
    /// assert_eq!(buf.info.size, 4);
    /// assert_eq!(Buffer::mapped_slice(&buf), &DATA);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn create_from_slice(
        device: &Arc<Device>,
        usage: vk::BufferUsageFlags,
        slice: impl AsRef<[u8]>,
    ) -> Result<Self, DriverError> {
        let slice = slice.as_ref();
        let info = BufferInfo::host_mem(slice.len() as _, usage);
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
    /// # use screen_13::driver::{AccessType, DriverError};
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # const SIZE: vk::DeviceSize = 1024;
    /// # let info = BufferInfo::device_mem(SIZE, vk::BufferUsageFlags::STORAGE_BUFFER);
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
    #[profiling::function]
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
    /// Panics if the buffer was not created with the `mappable` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # let info = BufferInfo::host_mem(4, vk::BufferUsageFlags::empty());
    /// # let mut my_buf = Buffer::create(&device, info)?;
    /// const DATA: [u8; 4] = [0xde, 0xad, 0xc0, 0xde];
    /// Buffer::copy_from_slice(&mut my_buf, 0, &DATA);
    ///
    /// assert_eq!(Buffer::mapped_slice(&my_buf), &DATA);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn copy_from_slice(this: &mut Self, offset: vk::DeviceSize, slice: impl AsRef<[u8]>) {
        let slice = slice.as_ref();
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
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # let info = BufferInfo::host_mem(4, vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);
    /// # let my_buf = Buffer::create(&device, info)?;
    /// let addr = Buffer::device_address(&my_buf);
    ///
    /// assert_ne!(addr, 0);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
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
    /// Panics if the buffer was not created with the `mappable` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # const DATA: [u8; 4] = [0; 4];
    /// # let my_buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::empty(), &DATA)?;
    /// // my_buf is mappable and filled with four zeroes
    /// let data = Buffer::mapped_slice(&my_buf);
    ///
    /// assert_eq!(data.len(), 4);
    /// assert_eq!(data[0], 0x00);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn mapped_slice(this: &Self) -> &[u8] {
        debug_assert!(
            this.info.mappable,
            "Buffer is not mappable - create using mappable flag"
        );

        &this.allocation.mapped_slice().unwrap()[0..this.info.size as usize]
    }

    /// Returns a mapped mutable slice.
    ///
    /// # Panics
    ///
    /// Panics if the buffer was not created with the `mappable` flag set to `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use glam::Mat4;
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::buffer::{Buffer, BufferInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # const DATA: [u8; 4] = [0; 4];
    /// # let mut my_buf = Buffer::create_from_slice(&device, vk::BufferUsageFlags::empty(), &DATA)?;
    /// let mut data = Buffer::mapped_slice_mut(&mut my_buf);
    /// data.copy_from_slice(&42f32.to_be_bytes());
    ///
    /// assert_eq!(data.len(), 4);
    /// assert_eq!(data[0], 0x42);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn mapped_slice_mut(this: &mut Self) -> &mut [u8] {
        debug_assert!(
            this.info.mappable,
            "Buffer is not mappable - create using mappable flag"
        );

        &mut this.allocation.mapped_slice_mut().unwrap()[0..this.info.size as usize]
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
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        {
            profiling::scope!("deallocate");

            #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
            let mut allocator = self.device.allocator.lock();

            #[cfg(not(feature = "parking_lot"))]
            let mut allocator = allocator.unwrap();

            allocator.free(unsafe { ManuallyDrop::take(&mut self.allocation) })
        }
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
    derive(Clone, Copy, Debug),
    pattern = "owned"
)]
#[non_exhaustive]
pub struct BufferInfo {
    /// Byte alignment of the base device address of the buffer.
    ///
    /// Must be a power of two.
    #[builder(default = "1")]
    pub alignment: vk::DeviceSize,

    /// Specifies a buffer whose memory is host visible and may be mapped.
    #[builder(default)]
    pub mappable: bool,

    /// Size in bytes of the buffer to be created.
    pub size: vk::DeviceSize,

    /// A bitmask of specifying allowed usages of the buffer.
    #[builder(default)]
    pub usage: vk::BufferUsageFlags,
}

impl BufferInfo {
    /// Specifies a non-mappable buffer with the given `size` and `usage` values.
    ///
    /// Device-local memory (located on the GPU) is used.
    #[inline(always)]
    pub const fn device_mem(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfo {
        BufferInfo {
            alignment: 1,
            mappable: false,
            size,
            usage,
        }
    }

    /// Specifies a mappable buffer with the given `size` and `usage` values.
    ///
    /// Host-local memory (located in CPU-accesible RAM) is used.
    ///
    /// # Note
    ///
    /// For convenience the given usage value will be bitwise OR'd with
    /// `TRANSFER_DST | TRANSFER_SRC`.
    #[inline(always)]
    pub const fn host_mem(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfo {
        let usage = vk::BufferUsageFlags::from_raw(
            usage.as_raw()
                | vk::BufferUsageFlags::TRANSFER_DST.as_raw()
                | vk::BufferUsageFlags::TRANSFER_SRC.as_raw(),
        );

        BufferInfo {
            alignment: 1,
            mappable: true,
            size,
            usage,
        }
    }

    /// Specifies a non-mappable buffer with the given `size` and `usage` values.
    #[allow(clippy::new_ret_no_self)]
    #[deprecated = "Use BufferInfo::device_mem()"]
    #[doc(hidden)]
    pub fn new(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        Self::device_mem(size, usage).to_builder()
    }

    /// Specifies a mappable buffer with the given `size` and `usage` values.
    ///
    /// # Note
    ///
    /// For convenience the given usage value will be bitwise OR'd with
    /// `TRANSFER_DST | TRANSFER_SRC`.
    #[deprecated = "Use BufferInfo::host_mem()"]
    #[doc(hidden)]
    pub fn new_mappable(size: vk::DeviceSize, usage: vk::BufferUsageFlags) -> BufferInfoBuilder {
        Self::host_mem(size, usage).to_builder()
    }

    /// Converts a `BufferInfo` into a `BufferInfoBuilder`.
    #[inline(always)]
    pub fn to_builder(self) -> BufferInfoBuilder {
        BufferInfoBuilder {
            alignment: Some(self.alignment),
            mappable: Some(self.mappable),
            size: Some(self.size),
            usage: Some(self.usage),
        }
    }
}

impl BufferInfoBuilder {
    /// Builds a new `BufferInfo`.
    ///    
    /// # Panics
    ///
    /// If any of the following values have not been set this function will panic:
    ///
    /// * `size`
    ///
    /// If `alignment` is not a power to two this function will panic.
    #[inline(always)]
    pub fn build(self) -> BufferInfo {
        let res = match self.fallible_build() {
            Err(BufferInfoBuilderError(err)) => panic!("{err}"),
            Ok(info) => info,
        };

        assert_eq!(
            res.alignment.count_ones(),
            1,
            "Alignment must be a power of two"
        );

        res
    }
}

impl From<BufferInfoBuilder> for BufferInfo {
    fn from(info: BufferInfoBuilder) -> Self {
        info.build()
    }
}

#[derive(Debug)]
struct BufferInfoBuilderError(UninitializedFieldError);

impl From<UninitializedFieldError> for BufferInfoBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(err)
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

#[cfg(test)]
mod tests {
    use super::*;

    type Info = BufferInfo;
    type Builder = BufferInfoBuilder;

    #[test]
    pub fn buffer_info() {
        let info = Info::device_mem(0, vk::BufferUsageFlags::empty());
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn buffer_info_alignment() {
        let info = Info::device_mem(0, vk::BufferUsageFlags::empty());

        assert_eq!(info.alignment, 1);
    }

    #[test]
    pub fn buffer_info_builder() {
        let info = Info::device_mem(0, vk::BufferUsageFlags::empty());
        let builder = Builder::default().size(0).build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Alignment must be a power of two")]
    pub fn buffer_info_builder_alignment_0() {
        Builder::default().size(0).alignment(0).build();
    }

    #[test]
    #[should_panic(expected = "Alignment must be a power of two")]
    pub fn buffer_info_builder_alignment_42() {
        Builder::default().size(0).alignment(42).build();
    }

    #[test]
    pub fn buffer_info_builder_alignment_256() {
        let mut info = Info::device_mem(42, vk::BufferUsageFlags::empty());
        info.alignment = 256;

        let builder = Builder::default().size(42).alignment(256).build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: size")]
    pub fn buffer_info_builder_uninit_size() {
        Builder::default().build();
    }
}
