use {
    super::{Buffer, BufferInfo, Device, DriverError},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    log::{info, warn},
    std::{ops::Deref, thread::panicking},
};

// #[derive(Clone, Debug)]
// pub struct RayTraceAccelerationScratchBuffer<P>
// where
//     P: SharedPointerKind,
// {
//     device: SharedPointer<Device<P>, P>,
//     buf: SharedPointer<Mutex<Buffer<P>>, P>,
// }

// impl<P> RayTraceAccelerationScratchBuffer<P>
// where
//     P: SharedPointerKind,
// {
//     pub const RT_SCRATCH_BUFFER_SIZE: vk::DeviceSize = 1024 * 1024 * 1440;

//     pub fn create(device: &SharedPointer<Device<P>, P>) -> Result<Self, DriverError> {
//         trace!("create");

//         let device = SharedPointer::clone(device);
//         let buf = SharedPointer::new(Mutex::new(Buffer::create(
//             &device,
//             BufferInfo::new(
//                 Self::RT_SCRATCH_BUFFER_SIZE,
//                 vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
//             )
//             .build(),
//         )?));

//         Ok(Self { device, buf })
//     }

//     // pub fn map_instances<'a>(
//     //     &'a self,
//     //     instances: impl Iterator<Item = RayTraceInstanceInfo<P>> + 'a,
//     // ) -> impl Iterator<Item = RayTraceInstance> + 'a {
//     //     instances.map(|desc| {
//     //         let blas_address = unsafe {
//     //             self.device
//     //                 .accel_struct_ext
//     //                 .as_ref()
//     //                 .unwrap()
//     //                 .get_acceleration_structure_device_address(
//     //                     &vk::AccelerationStructureDeviceAddressInfoKHR::builder()
//     //                         .acceleration_structure(desc.blas.accel_struct)
//     //                         .build(),
//     //                 )
//     //         };
//     //         let transform: [f32; 12] = [
//     //             desc.rotation[0], //.x_axis.x,
//     //             desc.rotation[0], //.y_axis.x,
//     //             desc.rotation[0], //.z_axis.x,
//     //             desc.position[0], //.x,
//     //             desc.rotation[0], //.x_axis.y,
//     //             desc.rotation[0], //.y_axis.y,
//     //             desc.rotation[0], //.z_axis.y,
//     //             desc.position[0], //.y,
//     //             desc.rotation[0], //.x_axis.z,
//     //             desc.rotation[0], //.y_axis.z,
//     //             desc.rotation[0], //.z_axis.z,
//     //             desc.position[0], //.z,
//     //         ];

//     //         RayTraceInstance::new(
//     //             transform,
//     //             desc.mesh_idx,
//     //             u8::MAX,
//     //             0,
//     //             vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
//     //             blas_address,
//     //         )
//     //     })
//     // }

//     pub fn rebuild_tlas(
//         &self,
//         cb: vk::CommandBuffer,
//         inst_buf_address: vk::DeviceAddress,
//         instance_count: usize,
//         tlas: &AccelerationStructure<P>,
//     ) {
//         use std::slice::from_ref;

//         // _build_range_infos: &[vk::AccelerationStructureBuildRangeInfoKHR],
//         // let scratch_buf = self.buf.lock();

//         // // See `RT_SCRATCH_BUFFER_SIZE`
//         // assert!(
//         //     mem_requirements.build_scratch_size <= scratch_buf.info.size,
//         //     "todo: resize scratch"
//         // );

//         // geometry_info.dst_acceleration_structure = accel_struct;
//         // geometry_info.scratch_data = vk::DeviceOrHostAddressKHR {
//         //     device_address: Buffer::device_address(&scratch_buf),
//         // };

//         // self.with_setup_cb(|cb| {
//         //     self.acceleration_structure_ext
//         //         .cmd_build_acceleration_structures(
//         //             cb,
//         //             from_ref(&geometry_info),
//         //             from_ref(&build_range_infos),
//         //         );

//         //     self.raw.cmd_pipeline_barrier(
//         //         cb,
//         //         vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
//         //         vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
//         //         vk::DependencyFlags::empty(),
//         //         &[vk::MemoryBarrier::builder()
//         //             .src_access_mask(
//         //                 vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
//         //                     | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
//         //             )
//         //             .dst_access_mask(
//         //                 vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
//         //                     | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
//         //             )
//         //             .build()],
//         //         &[],
//         //         &[],
//         //     );
//         // });

//         let geometry = vk::AccelerationStructureGeometryKHR::builder()
//             .geometry_type(vk::GeometryTypeKHR::INSTANCES)
//             .geometry(vk::AccelerationStructureGeometryDataKHR {
//                 instances: vk::AccelerationStructureGeometryInstancesDataKHR::builder()
//                     .data(vk::DeviceOrHostAddressConstKHR {
//                         device_address: inst_buf_address,
//                     })
//                     .build(),
//             })
//             .build();
//         let build_range_infos = vec![vk::AccelerationStructureBuildRangeInfoKHR::builder()
//             .primitive_count(instance_count as _)
//             .build()];
//         let mut geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
//             .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
//             .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
//             .geometries(from_ref(&geometry))
//             .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
//             .build();

//         let mem_requirements = unsafe {
//             self.device
//                 .accel_struct_ext
//                 .as_ref()
//                 .unwrap()
//                 .get_acceleration_structure_build_sizes(
//                     vk::AccelerationStructureBuildTypeKHR::DEVICE,
//                     &geometry_info,
//                     from_ref(&(instance_count as u32)),
//                 )
//         };
//         let scratch_buf = self.buf.lock();

//         assert!(
//             mem_requirements.acceleration_structure_size <= scratch_buf.info.size,
//             "todo: backing"
//         );
//         assert!(
//             mem_requirements.build_scratch_size <= scratch_buf.info.size,
//             "todo: scratch"
//         );

//         unsafe {
//             geometry_info.dst_acceleration_structure = tlas.accel_struct;
//             geometry_info.scratch_data = vk::DeviceOrHostAddressKHR {
//                 device_address: Buffer::device_address(&scratch_buf),
//             };

//             self.device
//                 .accel_struct_ext
//                 .as_ref()
//                 .unwrap()
//                 .cmd_build_acceleration_structures(
//                     cb,
//                     from_ref(&geometry_info),
//                     from_ref(&build_range_infos.as_slice()),
//                 );
//             self.device.cmd_pipeline_barrier(
//                 cb,
//                 vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
//                 vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
//                 vk::DependencyFlags::empty(),
//                 &[vk::MemoryBarrier::builder()
//                     .src_access_mask(
//                         vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
//                             | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
//                     )
//                     .dst_access_mask(
//                         vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR
//                             | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
//                     )
//                     .build()],
//                 &[],
//                 &[],
//             );
//         }
//     }
// }

#[derive(Debug)]
pub struct AccelerationStructure<P>
where
    P: SharedPointerKind,
{
    accel_struct: vk::AccelerationStructureKHR,
    pub buffer: Buffer<P>,
    device: SharedPointer<Device<P>, P>,
    pub info: AccelerationStructureInfo,
}

impl<P> AccelerationStructure<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &SharedPointer<Device<P>, P>,
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

        let device = SharedPointer::clone(device);

        Ok(AccelerationStructure {
            accel_struct,
            buffer,
            device,
            info,
        })
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

    pub fn size_of<'g>(
        device: &SharedPointer<Device<P>, P>,
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
                    &AccelerationStructureGeometryData::Instances { array_of_pointers } => (
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
                                    Some(DeviceOrHostAddress::HostAddress()) => {
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
                tls.max_primitive_counts.push(info.count);
            }

            let info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                .ty(info.ty)
                .flags(info.flags)
                .geometries(&tls.geometries);
            let sizes = unsafe {
                device
                    .accel_struct_ext
                    .as_ref()
                    .unwrap()
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

impl<P> Deref for AccelerationStructure<P>
where
    P: SharedPointerKind,
{
    type Target = vk::AccelerationStructureKHR;

    fn deref(&self) -> &Self::Target {
        &self.accel_struct
    }
}

impl<P> Drop for AccelerationStructure<P>
where
    P: SharedPointerKind,
{
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
    pub count: u32,
    pub flags: vk::GeometryFlagsKHR,
    pub geometry: AccelerationStructureGeometryData,
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
    },
    Triangles {
        index_type: vk::IndexType,
        max_vertex: u32,
        transform_data: Option<DeviceOrHostAddress>, // could be a bool during size call
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

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl AccelerationStructureInfoBuilder {
    pub fn build(self) -> AccelerationStructureInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
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
    HostAddress(), // TODO
}
