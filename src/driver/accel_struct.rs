use {
    super::{access_type_from_u8, access_type_into_u8, Buffer, BufferInfo, Device, DriverError},
    ash::vk,
    derive_builder::Builder,
    log::warn,
    std::{
        mem::size_of,
        ops::Deref,
        sync::{
            atomic::{AtomicU8, Ordering},
            Arc,
        },
        thread::panicking,
    },
    vk_sync::AccessType,
};

#[derive(Debug)]
pub struct AccelerationStructure {
    accel_struct: vk::AccelerationStructureKHR,
    pub buffer: Buffer,
    device: Arc<Device>,
    pub info: AccelerationStructureInfo,
    prev_access: AtomicU8,
}

impl AccelerationStructure {
    pub fn create(
        device: &Arc<Device>,
        info: impl Into<AccelerationStructureInfo>,
    ) -> Result<Self, DriverError> {
        let info = info.into();

        let buffer = Buffer::create(
            device,
            BufferInfo::new_mappable(
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

    pub fn access(this: &Self, next_access: AccessType) -> AccessType {
        access_type_from_u8(
            this.prev_access
                .swap(access_type_into_u8(next_access), Ordering::Relaxed),
        )
    }

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

    pub fn instance_slice<'a>(instance: vk::AccelerationStructureInstanceKHR) -> &'a [u8] {
        unsafe {
            std::slice::from_raw_parts(
                &instance as *const _ as *const _,
                size_of::<vk::AccelerationStructureInstanceKHR>(),
            )
        }
    }

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

        TLS.with(|tls| {
            let mut tls = tls.borrow_mut();
            tls.geometries.clear();
            tls.max_primitive_counts.clear();

            for info in info.geometries.iter() {
                let flags = info.flags;
                let (geometry_type, geometry) = match &info.geometry {
                    &AccelerationStructureGeometryData::AABBs { stride } => (
                        vk::GeometryTypeKHR::AABBS,
                        vk::AccelerationStructureGeometryDataKHR {
                            aabbs: vk::AccelerationStructureGeometryAabbsDataKHR {
                                stride,
                                ..Default::default()
                            },
                        },
                    ),
                    &AccelerationStructureGeometryData::Instances {
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
                    &AccelerationStructureGeometryData::Triangles {
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AccelerationStructureGeometry {
    pub max_primitive_count: u32,
    pub flags: vk::GeometryFlagsKHR,
    pub geometry: AccelerationStructureGeometryData,
}

impl AccelerationStructureGeometry {
    pub fn into_vk(&self) -> vk::AccelerationStructureGeometryKHR {
        let (geometry_type, geometry) = match &self.geometry {
            &AccelerationStructureGeometryData::AABBs { stride } => (
                vk::GeometryTypeKHR::AABBS,
                vk::AccelerationStructureGeometryDataKHR {
                    aabbs: vk::AccelerationStructureGeometryAabbsDataKHR {
                        stride,
                        ..Default::default()
                    },
                },
            ),
            &AccelerationStructureGeometryData::Instances {
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
            &AccelerationStructureGeometryData::Triangles {
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AccelerationStructureGeometryInfo {
    pub ty: vk::AccelerationStructureTypeKHR,
    pub flags: vk::BuildAccelerationStructureFlagsKHR,
    pub geometries: Vec<AccelerationStructureGeometry>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AccelerationStructureGeometryData {
    AABBs {
        stride: vk::DeviceSize,
    },
    Instances {
        array_of_pointers: bool,
        data: DeviceOrHostAddress,
    },
    Triangles {
        index_data: DeviceOrHostAddress,
        index_type: vk::IndexType,
        max_vertex: u32,
        transform_data: Option<DeviceOrHostAddress>,
        vertex_data: DeviceOrHostAddress,
        vertex_format: vk::Format,
        vertex_stride: vk::DeviceSize,
    },
}

#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build"),
    derive(Debug),
    pattern = "owned"
)]
pub struct AccelerationStructureInfo {
    pub ty: vk::AccelerationStructureTypeKHR,
    pub size: vk::DeviceSize,
}

impl AccelerationStructureInfo {
    pub const fn new_blas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            size,
        }
    }

    pub const fn new_tlas(size: vk::DeviceSize) -> Self {
        Self {
            ty: vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
            size,
        }
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl AccelerationStructureInfoBuilder {
    pub fn build(self) -> AccelerationStructureInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

// TODO: Remove this
impl From<AccelerationStructureInfo> for () {
    fn from(_: AccelerationStructureInfo) -> Self {}
}

#[derive(Clone, Copy, Debug)]
pub struct AccelerationStructureSize {
    pub create_size: vk::DeviceSize,
    pub update_size: vk::DeviceSize,
    pub build_size: vk::DeviceSize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeviceOrHostAddress {
    DeviceAddress(vk::DeviceAddress),
    HostAddress, // TODO
}

impl From<vk::DeviceAddress> for DeviceOrHostAddress {
    fn from(device_addr: vk::DeviceAddress) -> Self {
        Self::DeviceAddress(device_addr)
    }
}
