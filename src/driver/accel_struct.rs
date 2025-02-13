//! Acceleration structure resource types

use {
    super::{device::Device, Buffer, BufferInfo, DriverError},
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    log::warn,
    std::{
        ffi::c_void,
        mem::{replace, size_of_val},
        ops::Deref,
        sync::Arc,
        thread::panicking,
    },
    vk_sync::AccessType,
};

#[cfg(feature = "parking_lot")]
use parking_lot::Mutex;

#[cfg(not(feature = "parking_lot"))]
use std::sync::Mutex;

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
/// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
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
    access: Mutex<AccessType>,
    accel_struct: (vk::AccelerationStructureKHR, Buffer),
    device: Arc<Device>,

    /// Information used to create this object.
    pub info: AccelerationStructureInfo,
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
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
        debug_assert!(device.physical_device.accel_struct_properties.is_some());

        let info = info.into();

        let buffer = Buffer::create(
            device,
            BufferInfo::device_mem(
                info.size,
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            ),
        )?;

        let accel_struct = {
            let create_info = vk::AccelerationStructureCreateInfoKHR::default()
                .ty(info.ty)
                .buffer(*buffer)
                .size(info.size);

            let accel_struct_ext = Device::expect_accel_struct_ext(device);

            unsafe { accel_struct_ext.create_acceleration_structure(&create_info, None) }.map_err(
                |err| {
                    warn!("{err}");

                    match err {
                        vk::Result::ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS => {
                            DriverError::InvalidData
                        }
                        vk::Result::ERROR_OUT_OF_HOST_MEMORY => DriverError::OutOfMemory,
                        _ => DriverError::Unsupported,
                    }
                },
            )?
        };

        let device = Arc::clone(device);

        Ok(AccelerationStructure {
            access: Mutex::new(AccessType::Nothing),
            accel_struct: (accel_struct, buffer),
            device,
            info,
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
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
    pub fn access(this: &Self, access: AccessType) -> AccessType {
        #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
        let mut access_guard = this.access.lock();

        #[cfg(not(feature = "parking_lot"))]
        let mut access_guard = access_guard.unwrap();

        replace(&mut access_guard, access)
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
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
        let accel_struct_ext = Device::expect_accel_struct_ext(&this.device);

        unsafe {
            accel_struct_ext.get_acceleration_structure_device_address(
                &vk::AccelerationStructureDeviceAddressInfoKHR::default()
                    .acceleration_structure(this.accel_struct.0),
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
    /// # let my_geom_triangles = AccelerationStructureGeometryData::Triangles {
    /// #     index_addr: DeviceOrHostAddress::DeviceAddress(0),
    /// #     index_type: vk::IndexType::UINT32,
    /// #     max_vertex: 1,
    /// #     transform_addr: None,
    /// #     vertex_addr: DeviceOrHostAddress::DeviceAddress(0),
    /// #     vertex_format: vk::Format::R32G32B32_SFLOAT,
    /// #     vertex_stride: 12,
    /// # };
    /// let my_geom = AccelerationStructureGeometry {
    ///     max_primitive_count: 1,
    ///     flags: vk::GeometryFlagsKHR::OPAQUE,
    ///     geometry: my_geom_triangles,
    /// };
    /// let build_range = vk::AccelerationStructureBuildRangeInfoKHR {
    ///     primitive_count: 1,
    ///     primitive_offset: 0,
    ///     first_vertex: 0,
    ///     transform_offset: 0,
    /// };
    /// let my_info = AccelerationStructureGeometryInfo::blas([(my_geom, build_range)]);
    /// let res = AccelerationStructure::size_of(&device, &my_info);
    ///
    /// assert_eq!(res.create_size, 2432);
    /// assert_eq!(res.build_size, 640);
    /// assert_eq!(res.update_size, 0);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn size_of(
        device: &Device,
        info: &AccelerationStructureGeometryInfo<impl AsRef<AccelerationStructureGeometry>>,
    ) -> AccelerationStructureSize {
        use std::cell::RefCell;

        #[derive(Default)]
        struct Tls {
            geometries: Vec<vk::AccelerationStructureGeometryKHR<'static>>,
            max_primitive_counts: Vec<u32>,
        }

        thread_local! {
            static TLS: RefCell<Tls> = Default::default();
        }

        TLS.with_borrow_mut(|tls| {
            tls.geometries.clear();
            tls.max_primitive_counts.clear();

            for info in info.geometries.iter().map(AsRef::as_ref) {
                tls.geometries.push(info.into());
                tls.max_primitive_counts.push(info.max_primitive_count);
            }

            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(info.ty)
                .flags(info.flags)
                .geometries(&tls.geometries);
            let mut sizes = vk::AccelerationStructureBuildSizesInfoKHR::default();
            let accel_struct_ext = Device::expect_accel_struct_ext(device);

            unsafe {
                accel_struct_ext.get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::HOST_OR_DEVICE,
                    &info,
                    &tls.max_primitive_counts,
                    &mut sizes,
                );
            }

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
        &self.accel_struct.0
    }
}

impl Drop for AccelerationStructure {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        let accel_struct_ext = Device::expect_accel_struct_ext(&self.device);

        unsafe {
            accel_struct_ext.destroy_acceleration_structure(self.accel_struct.0, None);
        }
    }
}

/// Structure specifying geometries to be built into an acceleration structure.
///
/// See
/// [VkAccelerationStructureGeometryKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryKHR.html)
/// for more information.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationStructureGeometry {
    /// The number of primitives built into each geometry.
    pub max_primitive_count: u32,

    /// Describes additional properties of how the geometry should be built.
    pub flags: vk::GeometryFlagsKHR,

    /// Specifies acceleration structure geometry data.
    pub geometry: AccelerationStructureGeometryData,
}

impl AccelerationStructureGeometry {
    /// Creates a new acceleration structure geometry instance.
    pub fn new(max_primitive_count: u32, geometry: AccelerationStructureGeometryData) -> Self {
        let flags = Default::default();

        Self {
            max_primitive_count,
            flags,
            geometry,
        }
    }

    /// Creates a new acceleration structure geometry instance with the
    /// [vk::GeometryFlagsKHR::OPAQUE] flag set.
    pub fn opaque(max_primitive_count: u32, geometry: AccelerationStructureGeometryData) -> Self {
        Self::new(max_primitive_count, geometry).flags(vk::GeometryFlagsKHR::OPAQUE)
    }

    /// Sets the instance flags.
    pub fn flags(mut self, flags: vk::GeometryFlagsKHR) -> Self {
        self.flags = flags;

        self
    }
}

impl<T> AsRef<AccelerationStructureGeometry> for (AccelerationStructureGeometry, T) {
    fn as_ref(&self) -> &AccelerationStructureGeometry {
        &self.0
    }
}

impl<'b> From<&'b AccelerationStructureGeometry> for vk::AccelerationStructureGeometryKHR<'_> {
    fn from(&value: &'b AccelerationStructureGeometry) -> Self {
        value.into()
    }
}

impl From<AccelerationStructureGeometry> for vk::AccelerationStructureGeometryKHR<'_> {
    fn from(value: AccelerationStructureGeometry) -> Self {
        Self::default()
            .flags(value.flags)
            .geometry(value.geometry.into())
            .geometry_type(value.geometry.into())
    }
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
        /// A device or host address to memory containing [vk::AabbPositionsKHR] structures
        /// containing position data for each axis-aligned bounding box in the geometry.
        addr: DeviceOrHostAddress,

        /// Stride in bytes between each entry in data.
        ///
        /// The stride must be a multiple of `8`.
        stride: vk::DeviceSize,
    },

    /// Geometry consisting of instances of other acceleration structures.
    ///
    /// See [VkAccelerationStructureGeometryInstancesDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryInstancesDataKHR.html)
    /// for more information.
    Instances {
        /// Either the address of an array of device referencing individual
        /// VkAccelerationStructureInstanceKHR structures or packed motion instance information as
        /// described in
        /// [motion instances](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#acceleration-structure-motion-instances)
        /// if `array_of_pointers` is `true`, or the address of an array of
        /// VkAccelerationStructureInstanceKHR structures.
        ///
        /// Addresses and VkAccelerationStructureInstanceKHR structures are tightly packed.
        addr: DeviceOrHostAddress,

        /// Specifies whether data is used as an array of addresses or just an array.
        array_of_pointers: bool,
    },

    /// A triangle geometry in a bottom-level acceleration structure.
    ///
    /// See
    /// [VkAccelerationStructureGeometryTrianglesDataKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkAccelerationStructureGeometryTrianglesDataKHR.html)
    /// for more information.
    Triangles {
        /// A device or host address to memory containing index data for this geometry.
        index_addr: DeviceOrHostAddress,

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
        transform_addr: Option<DeviceOrHostAddress>,

        /// A device or host address to memory containing vertex data for this geometry.
        vertex_addr: DeviceOrHostAddress,

        /// The
        /// [VkFormat](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkFormat.html)
        /// of each vertex element.
        vertex_format: vk::Format,

        /// The stride in bytes between each vertex.
        vertex_stride: vk::DeviceSize,
    },
}

impl AccelerationStructureGeometryData {
    /// Specifies acceleration structure geometry data as AABBs.
    pub fn aabbs(addr: impl Into<DeviceOrHostAddress>, stride: vk::DeviceSize) -> Self {
        let addr = addr.into();

        Self::AABBs { addr, stride }
    }

    /// Specifies acceleration structure geometry data as instances.
    pub fn instances(addr: impl Into<DeviceOrHostAddress>) -> Self {
        let addr = addr.into();

        Self::Instances {
            addr,
            array_of_pointers: false,
        }
    }

    /// Specifies acceleration structure geometry data as an array of instance pointers.
    pub fn instance_pointers(addr: impl Into<DeviceOrHostAddress>) -> Self {
        let addr = addr.into();

        Self::Instances {
            addr,
            array_of_pointers: true,
        }
    }

    /// Specifies acceleration structure geometry data as triangles.
    pub fn triangles(
        index_addr: impl Into<DeviceOrHostAddress>,
        index_type: vk::IndexType,
        max_vertex: u32,
        transform_addr: impl Into<Option<DeviceOrHostAddress>>,
        vertex_addr: impl Into<DeviceOrHostAddress>,
        vertex_format: vk::Format,
        vertex_stride: vk::DeviceSize,
    ) -> Self {
        let index_addr = index_addr.into();
        let transform_addr = transform_addr.into();
        let vertex_addr = vertex_addr.into();

        Self::Triangles {
            index_addr,
            index_type,
            max_vertex,
            transform_addr,
            vertex_addr,
            vertex_format,
            vertex_stride,
        }
    }
}

impl From<AccelerationStructureGeometryData> for vk::GeometryTypeKHR {
    fn from(value: AccelerationStructureGeometryData) -> Self {
        match value {
            AccelerationStructureGeometryData::AABBs { .. } => Self::AABBS,
            AccelerationStructureGeometryData::Instances { .. } => Self::INSTANCES,
            AccelerationStructureGeometryData::Triangles { .. } => Self::TRIANGLES,
        }
    }
}

impl From<AccelerationStructureGeometryData> for vk::AccelerationStructureGeometryDataKHR<'_> {
    fn from(value: AccelerationStructureGeometryData) -> Self {
        match value {
            AccelerationStructureGeometryData::AABBs { addr, stride } => Self {
                aabbs: vk::AccelerationStructureGeometryAabbsDataKHR::default()
                    .data(addr.into())
                    .stride(stride),
            },
            AccelerationStructureGeometryData::Instances {
                addr,
                array_of_pointers,
            } => Self {
                instances: vk::AccelerationStructureGeometryInstancesDataKHR::default()
                    .array_of_pointers(array_of_pointers)
                    .data(addr.into()),
            },
            AccelerationStructureGeometryData::Triangles {
                index_addr,
                index_type,
                max_vertex,
                transform_addr,
                vertex_addr,
                vertex_format,
                vertex_stride,
            } => Self {
                triangles: vk::AccelerationStructureGeometryTrianglesDataKHR::default()
                    .index_data(index_addr.into())
                    .index_type(index_type)
                    .max_vertex(max_vertex)
                    .transform_data(transform_addr.map(Into::into).unwrap_or_default())
                    .vertex_data(vertex_addr.into())
                    .vertex_format(vertex_format)
                    .vertex_stride(vertex_stride),
            },
        }
    }
}

/// Specifies the geometry data of an acceleration structure.
#[derive(Clone, Debug)]
pub struct AccelerationStructureGeometryInfo<G> {
    /// Type of acceleration structure.
    pub ty: vk::AccelerationStructureTypeKHR,

    /// Specifies additional parameters of the acceleration structure.
    pub flags: vk::BuildAccelerationStructureFlagsKHR,

    /// A slice of geometry structures.
    pub geometries: Box<[G]>,
}

impl<G> AccelerationStructureGeometryInfo<G> {
    /// A bottom-level acceleration structure containing the AABBs or geometry to be intersected.
    pub fn blas(geometries: impl Into<Box<[G]>>) -> Self {
        let geometries = geometries.into();

        Self {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            flags: Default::default(),
            geometries,
        }
    }

    /// A top-level acceleration structure containing instance data referring to bottom-level
    /// acceleration structures.
    pub fn tlas(geometries: impl Into<Box<[G]>>) -> Self {
        let geometries = geometries.into();

        Self {
            ty: vk::AccelerationStructureTypeKHR::TOP_LEVEL,
            flags: Default::default(),
            geometries,
        }
    }

    /// Sets the flags on this instance.
    pub fn flags(mut self, flags: vk::BuildAccelerationStructureFlagsKHR) -> Self {
        self.flags = flags;
        self
    }
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

    /// Creates a default `AccelerationStructureInfoBuilder`.
    #[allow(clippy::new_ret_no_self)]
    pub fn builder() -> AccelerationStructureInfoBuilder {
        Default::default()
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
    /// The size of the scratch buffer required when building an acceleration structure using the
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
/// [VkDeviceOrHostAddressKHR](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkDeviceOrHostAddressKHR.html)
/// for more information.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeviceOrHostAddress {
    /// An address value returned from [`AccelerationStructure::device_address`].
    DeviceAddress(vk::DeviceAddress),

    /// A host memory address.
    HostAddress(*mut c_void),
}

impl From<vk::DeviceAddress> for DeviceOrHostAddress {
    fn from(device_address: vk::DeviceAddress) -> Self {
        Self::DeviceAddress(device_address)
    }
}

impl From<*mut c_void> for DeviceOrHostAddress {
    fn from(host_address: *mut c_void) -> Self {
        Self::HostAddress(host_address)
    }
}

// Safety: The entire purpose of DeviceOrHostAddress is to share memory with Vulkan
unsafe impl Send for DeviceOrHostAddress {}
unsafe impl Sync for DeviceOrHostAddress {}

impl From<DeviceOrHostAddress> for vk::DeviceOrHostAddressConstKHR {
    fn from(value: DeviceOrHostAddress) -> Self {
        match value {
            DeviceOrHostAddress::DeviceAddress(device_address) => Self { device_address },
            DeviceOrHostAddress::HostAddress(host_address) => Self { host_address },
        }
    }
}

impl From<DeviceOrHostAddress> for vk::DeviceOrHostAddressKHR {
    fn from(value: DeviceOrHostAddress) -> Self {
        match value {
            DeviceOrHostAddress::DeviceAddress(device_address) => Self { device_address },
            DeviceOrHostAddress::HostAddress(host_address) => Self { host_address },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Info = AccelerationStructureInfo;
    type Builder = AccelerationStructureInfoBuilder;

    #[test]
    pub fn accel_struct_info() {
        let info = Info::blas(32);
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn accel_struct_info_builder() {
        let info = Info {
            size: 32,
            ty: vk::AccelerationStructureTypeKHR::GENERIC,
        };
        let builder = Builder::default().size(32).build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: size")]
    pub fn accel_struct_info_builder_uninit_size() {
        Builder::default().build();
    }
}
