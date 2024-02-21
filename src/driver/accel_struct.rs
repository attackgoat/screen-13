//! Acceleration structure resource types

use {
    super::{
        access_type_from_u8, access_type_into_u8, device::Device, Buffer, BufferInfo, DriverError,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::warn,
    std::{
        mem::size_of_val,
        ops::Deref,
        sync::{
            atomic::{AtomicU8, Ordering},
            Arc,
        },
        thread::panicking,
    },
    vk_sync::AccessType,
};

/// Smart pointer handle to an [acceleration structure] object.
///
/// Also contains the backing buffer and information about the object.
///
/// ## `Deref` behavior
///
/// `AccelerationStructure` automatically dereferences to [`vk::AccelerationStructureKHR`] (via the
/// [`Deref`] trait), so you can call `vk::AccelerationStructureKHR`'s methods on a value of
/// type `AccelerationStructure`. To avoid name clashes with `vk::AccelerationStructureKHR`'s
/// methods, the methods of `AccelerationStructure` itself are associated functions, called using
/// [fully qualified syntax]:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{AccessType, DriverError};
/// # use screen_13::driver::device::{Device, DeviceInfo};
/// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureInfo};
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
/// # const SIZE: vk::DeviceSize = 1024;
/// # let info = AccelerationStructureInfo::blas(SIZE);
/// # let my_accel_struct = AccelerationStructure::create(&device, info)?;
/// let addr = AccelerationStructure::device_address(&my_accel_struct);
/// # Ok(()) }
/// ```
///
/// [acceleration structure]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureKHR.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
#[derive(Debug)]
pub struct AccelerationStructure {
    accel_struct: vk::AccelerationStructureKHR,

    /// Backing storage buffer for this object.
    pub buffer: Buffer,

    device: Arc<Device>,

    /// Information used to create this object.
    pub info: AccelerationStructureInfo,

    prev_access: AtomicU8,
}

impl AccelerationStructure {
    /// Creates a new acceleration structure on the given device.
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
    /// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// const SIZE: vk::DeviceSize = 1024;
    /// let info = AccelerationStructureInfo::blas(SIZE);
    /// let accel_struct = AccelerationStructure::create(&device, info)?;
    ///
    /// assert_ne!(*accel_struct, vk::AccelerationStructureKHR::null());
    /// assert_eq!(accel_struct.info.size, SIZE);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn create(
        device: &Arc<Device>,
        info: impl Into<AccelerationStructureInfo>,
    ) -> Result<Self, DriverError> {
        let info = info.into();

        let buffer = Buffer::create(
            device,
            BufferInfo::host_mem(
                info.size,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            ),
        )?;

        let accel_struct = {
            let create_info = vk::AccelerationStructureCreateInfoKHR::builder()
                .ty(info.ty)
                .buffer(*buffer)
                .size(info.size);

            unsafe {
                device
                    .accel_struct_ext
                    .as_ref()
                    .unwrap()
                    .create_acceleration_structure(&create_info, None)
                    .map_err(|err| {
                        warn!("{err}");

                        DriverError::Unsupported
                    })?
            }
        };

        let device = Arc::clone(device);

        Ok(AccelerationStructure {
            accel_struct,
            buffer,
            device,
            info,
            prev_access: AtomicU8::new(access_type_into_u8(AccessType::Nothing)),
        })
    }

    /// Keeps track of some `next_access` which affects this object.
    ///
    /// Returns the previous access for which a pipeline barrier should be used to prevent data
    /// corruption.
    ///
    /// # Note
    ///
    /// Used to maintain object state when passing a _Screen 13_-created
    /// `vk::AccelerationStructureKHR` handle to external code such as [_Ash_] or [_Erupt_]
    /// bindings.
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
    /// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # const SIZE: vk::DeviceSize = 1024;
    /// # let info = AccelerationStructureInfo::blas(SIZE);
    /// # let my_accel_struct = AccelerationStructure::create(&device, info)?;
    /// // Initially we want to "Build Write"
    /// let next = AccessType::AccelerationStructureBuildWrite;
    /// let prev = AccelerationStructure::access(&my_accel_struct, next);
    /// assert_eq!(prev, AccessType::Nothing);
    ///
    /// // External code may now "Build Write"; no barrier required
    ///
    /// // Subsequently we want to "Build Read"
    /// let next = AccessType::AccelerationStructureBuildRead;
    /// let prev = AccelerationStructure::access(&my_accel_struct, next);
    /// assert_eq!(prev, AccessType::AccelerationStructureBuildWrite);
    ///
    /// // A barrier on "Build Write" before "Build Read" is required!
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

    /// Returns the device address of this object.
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
    /// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # const SIZE: vk::DeviceSize = 1024;
    /// # let info = AccelerationStructureInfo::blas(SIZE);
    /// # let my_accel_struct = AccelerationStructure::create(&device, info)?;
    /// let addr = AccelerationStructure::device_address(&my_accel_struct);
    ///
    /// assert_ne!(addr, 0);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn device_address(this: &Self) -> vk::DeviceAddress {
        unsafe {
            this.device
                .accel_struct_ext
                .as_ref()
                .unwrap()
                .get_acceleration_structure_device_address(
                    &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
                        .acceleration_structure(this.accel_struct),
                )
        }
    }

    /// Helper function which is used to prepare instance buffers.
    pub fn instance_slice(instances: &[vk::AccelerationStructureInstanceKHR]) -> &[u8] {
        use std::slice::from_raw_parts;

        unsafe { from_raw_parts(instances.as_ptr() as *const _, size_of_val(instances)) }
    }

    /// Returns the size of some geometry info which is then used to create a new
    /// [AccelerationStructure] instance or update an existing instance.
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
    /// # use screen_13::driver::accel_struct::{AccelerationStructure, AccelerationStructureGeometry, AccelerationStructureGeometryData, AccelerationStructureGeometryInfo, DeviceOrHostAddress};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # let my_geom = AccelerationStructureGeometryData::Triangles {
    /// #     index_data: DeviceOrHostAddress::DeviceAddress(0),
    /// #     index_type: vk::IndexType::UINT32,
    /// #     max_vertex: 1,
    /// #     transform_data: None,
    /// #     vertex_data: DeviceOrHostAddress::DeviceAddress(0),
    /// #     vertex_format: vk::Format::R32G32B32_SFLOAT,
    /// #     vertex_stride: 12,
    /// # };
    /// let my_info = AccelerationStructureGeometryInfo {
    ///     ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
    ///     flags: vk::BuildAccelerationStructureFlagsKHR::empty(),
    ///     geometries: vec![AccelerationStructureGeometry {
    ///         max_primitive_count: 1,
    ///         flags: vk::GeometryFlagsKHR::OPAQUE,
    ///         geometry: my_geom,
    ///     }],
    /// };
    /// let res = AccelerationStructure::size_of(&device, &my_info);
    ///
    /// assert_eq!(res.create_size, 2432);
    /// assert_eq!(res.build_size, 640);
    /// assert_eq!(res.update_size, 0);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn size_of(
        device: &Arc<Device>,
        info: &AccelerationStructureGeometryInfo,
    ) -> AccelerationStructureSize {
        use std::cell::RefCell;

        #[derive(Default)]
        struct Tls {
            geometries: Vec<vk::AccelerationStructureGeometryKHR>,
            max_primitive_counts: Vec<u32>,
        }

        thread_local! {
            static TLS: RefCell<Tls> = Default::default();
        }

        TLS.with_borrow_mut(|tls| {
            tls.geometries.clear();
            tls.max_primitive_counts.clear();

            for info in info.geometries.iter() {
                let flags = info.flags;
                let (geometry_type, geometry) = match info.geometry {
                    AccelerationStructureGeometryData::AABBs { stride } => (
                        vk::GeometryTypeKHR::AABBS,
                        vk::AccelerationStructureGeometryDataKHR {
                            aabbs: vk::AccelerationStructureGeometryAabbsDataKHR {
                                stride,
                                ..Default::default()
                            },
                        },
                    ),
                    AccelerationStructureGeometryData::Instances {
                        array_of_pointers, ..
                    } => (
                        vk::GeometryTypeKHR::INSTANCES,
                        vk::AccelerationStructureGeometryDataKHR {
                            instances: vk::AccelerationStructureGeometryInstancesDataKHR {
                                array_of_pointers: array_of_pointers as _,
                                ..Default::default()
                            },
                        },
                    ),
                    AccelerationStructureGeometryData::Triangles {
                        index_type,
                        max_vertex,
                        transform_data,
                        vertex_format,
                        vertex_stride,
                        ..
                    } => (
                        vk::GeometryTypeKHR::TRIANGLES,
                        vk::AccelerationStructureGeometryDataKHR {
                            triangles: vk::AccelerationStructureGeometryTrianglesDataKHR {
                                vertex_format,
                                vertex_stride,
                                max_vertex,
                                index_type,
                                transform_data: match transform_data {
                                    Some(DeviceOrHostAddress::DeviceAddress(device_address)) => {
                                        vk::DeviceOrHostAddressConstKHR { device_address }
                                    }
                                    Some(DeviceOrHostAddress::HostAddress) => {
                                        vk::DeviceOrHostAddressConstKHR {
                                            host_address: std::ptr::null(),
                                        } // TODO
                                    }
                                    None => vk::DeviceOrHostAddressConstKHR { device_address: 0 },
                                },
                                ..Default::default()
                            },
                        },
                    ),
                };

                tls.geometries.push(vk::AccelerationStructureGeometryKHR {
                    flags,
                    geometry_type,
                    geometry,
                    ..Default::default()
                });
                tls.max_primitive_counts.push(info.max_primitive_count);
            }

            let info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                .ty(info.ty)
                .flags(info.flags)
                .geometries(&tls.geometries);
            let sizes = unsafe {
                device
                    .accel_struct_ext
                    .as_ref()
                    .expect("ray tracing feature must be enabled")
                    .get_acceleration_structure_build_sizes(
                        vk::AccelerationStructureBuildTypeKHR::HOST_OR_DEVICE,
                        &info,
                        tls.max_primitive_counts.as_slice(),
                    )
            };

            AccelerationStructureSize {
                create_size: sizes.acceleration_structure_size,
                build_size: sizes.build_scratch_size,
                update_size: sizes.update_scratch_size,
            }
        })
    }
}

impl Deref for AccelerationStructure {
    type Target = vk::AccelerationStructureKHR;

    fn deref(&self) -> &Self::Target {
        &self.accel_struct
    }
}

impl Drop for AccelerationStructure {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device
                .accel_struct_ext
                .as_ref()
                .unwrap()
                .destroy_acceleration_structure(self.accel_struct, None);
        }
    }
}

/// Structure specifying geometries to be built into an acceleration structure.
///
/// See
/// [VkAccelerationStructureGeometryKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryKHR.html)
/// for more information.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccelerationStructureGeometry {
    /// The number of primitives built into each geometry.
    pub max_primitive_count: u32,

    /// Describes additional properties of how the geometry should be built.
    pub flags: vk::GeometryFlagsKHR,

    /// Specifies acceleration structure geometry data.
    pub geometry: AccelerationStructureGeometryData,
}

impl AccelerationStructureGeometry {
    pub(crate) fn into_vk(self) -> vk::AccelerationStructureGeometryKHR {
        let (geometry_type, geometry) = match self.geometry {
            AccelerationStructureGeometryData::AABBs { stride } => (
                vk::GeometryTypeKHR::AABBS,
                vk::AccelerationStructureGeometryDataKHR {
                    aabbs: vk::AccelerationStructureGeometryAabbsDataKHR {
                        stride,
                        ..Default::default()
                    },
                },
            ),
            AccelerationStructureGeometryData::Instances {
                array_of_pointers,
                data,
            } => (
                vk::GeometryTypeKHR::INSTANCES,
                vk::AccelerationStructureGeometryDataKHR {
                    instances: vk::AccelerationStructureGeometryInstancesDataKHR {
                        array_of_pointers: array_of_pointers as _,
                        data: match data {
                            DeviceOrHostAddress::DeviceAddress(device_address) => {
                                vk::DeviceOrHostAddressConstKHR { device_address }
                            }
                            DeviceOrHostAddress::HostAddress => todo!(),
                        },
                        ..Default::default()
                    },
                },
            ),
            AccelerationStructureGeometryData::Triangles {
                index_data,
                index_type,
                max_vertex,
                transform_data,
                vertex_data,
                vertex_format,
                vertex_stride,
            } => (
                vk::GeometryTypeKHR::TRIANGLES,
                vk::AccelerationStructureGeometryDataKHR {
                    triangles: vk::AccelerationStructureGeometryTrianglesDataKHR {
                        index_data: match index_data {
                            DeviceOrHostAddress::DeviceAddress(device_address) => {
                                vk::DeviceOrHostAddressConstKHR { device_address }
                            }
                            DeviceOrHostAddress::HostAddress => todo!(),
                        },
                        index_type,
                        max_vertex,
                        transform_data: match transform_data {
                            Some(DeviceOrHostAddress::DeviceAddress(device_address)) => {
                                vk::DeviceOrHostAddressConstKHR { device_address }
                            }
                            Some(DeviceOrHostAddress::HostAddress) => todo!(),
                            None => vk::DeviceOrHostAddressConstKHR { device_address: 0 },
                        },
                        vertex_data: match vertex_data {
                            DeviceOrHostAddress::DeviceAddress(device_address) => {
                                vk::DeviceOrHostAddressConstKHR { device_address }
                            }
                            DeviceOrHostAddress::HostAddress => todo!(),
                        },
                        vertex_format,
                        vertex_stride,
                        ..Default::default()
                    },
                },
            ),
        };
        let flags = self.flags;

        vk::AccelerationStructureGeometryKHR {
            flags,
            geometry_type,
            geometry,
            ..Default::default()
        }
    }
}

/// Specifies the geometry data used to build an acceleration structure.
///
/// See
/// [VkAccelerationStructureBuildGeometryInfoKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureBuildGeometryInfoKHR.html)
/// for more information.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AccelerationStructureGeometryInfo {
    /// Type of acceleration structure.
    pub ty: vk::AccelerationStructureTypeKHR,

    /// Specifies additional parameters of the acceleration structure.
    pub flags: vk::BuildAccelerationStructureFlagsKHR,

    /// An array of [AccelerationStructureGeometry] structures to be built.
    pub geometries: Vec<AccelerationStructureGeometry>,
}

/// Specifies acceleration structure geometry data.
///
/// See
/// [VkAccelerationStructureGeometryDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryDataKHR.html)
/// for more information.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccelerationStructureGeometryData {
    /// Axis-aligned bounding box geometry in a bottom-level acceleration structure.
    ///
    /// See
    /// [VkAccelerationStructureGeometryAabbsDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryAabbsDataKHR.html)
    /// for more information.
    AABBs {
        /// Stride in bytes between each entry in data.
        ///
        /// The stride must be a multiple of 8.
        stride: vk::DeviceSize,
    },

    /// Geometry consisting of instances of other acceleration structures.
    ///
    /// See [VkAccelerationStructureGeometryInstancesDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryInstancesDataKHR.html)
    /// for more information.
    Instances {
        /// Specifies whether data is used as an array of addresses or just an array.
        array_of_pointers: bool,

        /// Either the address of an array of device referencing individual
        /// VkAccelerationStructureInstanceKHR structures or packed motion instance information as
        /// described in
        /// [motion instances](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#acceleration-structure-motion-instances)
        /// if `array_of_pointers` is `true`, or the address of an array of
        /// VkAccelerationStructureInstanceKHR structures.
        ///
        /// Addresses and VkAccelerationStructureInstanceKHR structures are tightly packed.
        data: DeviceOrHostAddress,
    },

    /// A triangle geometry in a bottom-level acceleration structure.
    ///
    /// See
    /// [VkAccelerationStructureGeometryTrianglesDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryTrianglesDataKHR.html)
    /// for more information.
    Triangles {
        /// A device or host address to memory containing index data for this geometry.
        index_data: DeviceOrHostAddress,

        /// The
        /// [VkIndexType](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkIndexType.html)
        /// of each index element.
        index_type: vk::IndexType,

        /// The highest index of a vertex that will be addressed by a build command using this structure.
        max_vertex: u32,

        /// A device or host address to memory containing an optional reference to a
        /// [VkTransformMatrixKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkTransformMatrixKHR.html)
        /// structure describing a transformation from the space in which the vertices in this
        /// geometry are described to the space in which the acceleration structure is defined.
        transform_data: Option<DeviceOrHostAddress>,

        /// A device or host address to memory containing vertex data for this geometry.
        vertex_data: DeviceOrHostAddress,

        /// The
        /// [VkFormat](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkFormat.html)
        /// of each vertex element.
        vertex_format: vk::Format,

        /// The stride in bytes between each vertex.
        vertex_stride: vk::DeviceSize,
    },
}

/// Information used to create an [`AccelerationStructure`] instance.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(
        private,
        name = "fallible_build",
        error = "AccelerationStructureInfoBuilderError"
    ),
    derive(Clone, Copy, Debug),
    pattern = "owned"
)]
#[non_exhaustive]
pub struct AccelerationStructureInfo {
    /// Type of acceleration structure.
    #[builder(default = "vk::AccelerationStructureTypeKHR::GENERIC")]
    pub ty: vk::AccelerationStructureTypeKHR,

    /// The size of the backing buffer that will store the acceleration structure.
    ///
    /// Use [`AccelerationStructure::size_of`] to calculate this value.
    pub size: vk::DeviceSize,
}

impl AccelerationStructureInfo {
    /// Specifies a [`vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL`] acceleration structure of the
    /// given size.
    #[inline(always)]
    pub const fn blas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            size,
        }
    }

    /// Specifies a [`vk::AccelerationStructureTypeKHR::GENERIC`] acceleration structure of the
    /// given size.
    #[inline(always)]
    pub const fn generic(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::GENERIC,
            size,
        }
    }

    /// Specifies a [`vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL`] acceleration structure of the
    /// given size.
    #[deprecated = "Use AccelerationStructureInfo::blas()"]
    #[doc(hidden)]
    pub const fn new_blas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            size,
        }
    }

    /// Specifies a [`vk::AccelerationStructureTypeKHR::TOP_LEVEL`] acceleration structure of the
    /// given size.
    #[deprecated = "Use AccelerationStructureInfo::tlas()"]
    #[doc(hidden)]
    pub const fn new_tlas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
            size,
        }
    }

    /// Specifies a [`vk::AccelerationStructureTypeKHR::TOP_LEVEL`] acceleration structure of the
    /// given size.
    #[inline(always)]
    pub const fn tlas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
            size,
        }
    }

    /// Converts an `AccelerationStructureInfo` into an `AccelerationStructureInfoBuilder`.
    #[inline(always)]
    pub fn to_builder(self) -> AccelerationStructureInfoBuilder {
        AccelerationStructureInfoBuilder {
            ty: Some(self.ty),
            size: Some(self.size),
        }
    }
}

impl From<AccelerationStructureInfo> for () {
    fn from(_: AccelerationStructureInfo) -> Self {}
}

impl AccelerationStructureInfoBuilder {
    /// Builds a new `AccelerationStructureInfo`.
    ///
    /// # Panics
    ///
    /// If any of the following values have not been set this function will panic:
    ///
    /// * `size`
    #[inline(always)]
    pub fn build(self) -> AccelerationStructureInfo {
        match self.fallible_build() {
            Err(AccelerationStructureInfoBuilderError(err)) => panic!("{err}"),
            Ok(info) => info,
        }
    }
}

#[derive(Debug)]
struct AccelerationStructureInfoBuilderError(UninitializedFieldError);

impl From<UninitializedFieldError> for AccelerationStructureInfoBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(err)
    }
}

/// Holds the results of the [`AccelerationStructure::size_of`] function.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationStructureSize {
    /// The size of the scratch buffer required when updating an acceleration structure using the
    /// [`Acceleration::build_structure`](super::super::graph::pass_ref::Acceleration::build_structure)
    /// function.
    pub build_size: vk::DeviceSize,

    /// The value of `size` parameter needed by [`AccelerationStructureInfo`] for use with the
    /// [`AccelerationStructure::create`] function.
    pub create_size: vk::DeviceSize,

    /// The size of the scratch buffer required when updating an acceleration structure using the
    /// [`Acceleration::update_structure`](super::super::graph::pass_ref::Acceleration::update_structure)
    /// function.
    pub update_size: vk::DeviceSize,
}

/// Specifies a constant device or host address.
///
/// See
/// [VkDeviceOrHostAddressConstKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkDeviceOrHostAddressConstKHR.html)
/// for more information.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeviceOrHostAddress {
    /// An address value returned from [`AccelerationStructure::device_address`].
    DeviceAddress(vk::DeviceAddress),

    /// TODO: Not yet supported
    HostAddress,
}

impl From<vk::DeviceAddress> for DeviceOrHostAddress {
    fn from(device_addr: vk::DeviceAddress) -> Self {
        Self::DeviceAddress(device_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Info = AccelerationStructureInfo;
    type Builder = AccelerationStructureInfoBuilder;

    #[test]
    pub fn accel_struct_info() {
        let info = Info::generic(0);
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn accel_struct_info_builder() {
        let info = Info::generic(0);
        let builder = Builder::default().size(0).build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: size")]
    pub fn accel_struct_info_builder_uninit_size() {
        Builder::default().build();
    }
}
