use {
    super::{
        DriverConfig, DriverError, Instance, PhysicalDevice,
        PhysicalDeviceAccelerationStructureProperties, PhysicalDeviceDepthStencilResolveProperties,
        PhysicalDeviceRayQueryFeatures, PhysicalDeviceRayTracePipelineProperties,
        PhysicalDeviceRayTracingPipelineFeatures, PhysicalDeviceVulkan10Features,
        PhysicalDeviceVulkan11Features, PhysicalDeviceVulkan11Properties,
        PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan12Properties, Queue, SamplerDesc,
        Surface,
    },
    ash::{
        extensions::khr,
        vk::{self, InstanceFnV1_1},
    },
    gpu_allocator::{
        vulkan::{Allocator, AllocatorCreateDesc},
        AllocatorDebugSettings,
    },
    log::{debug, error, trace, warn},
    parking_lot::Mutex,
    std::{
        collections::{HashMap, HashSet},
        ffi::CStr,
        fmt::{Debug, Formatter},
        iter::{empty, repeat},
        mem::forget,
        ops::Deref,
        sync::Arc,
        thread::panicking,
        time::Instant,
    },
};

/// Opaque handle to a device object.
pub struct Device {
    pub(crate) accel_struct_ext: Option<khr::AccelerationStructure>,

    /// Describes the properties of the device which relate to acceleration structures, if
    /// available.
    pub accel_struct_properties: Option<PhysicalDeviceAccelerationStructureProperties>,

    pub(super) allocator: Option<Mutex<Allocator>>,

    /// Describes the properties of the device which relate to depth/stencil resolve operations.
    pub depth_stencil_resolve_properties: PhysicalDeviceDepthStencilResolveProperties,

    device: ash::Device,
    immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,

    /// Vulkan instance pointer, which includes useful functions.
    pub(crate) instance: Arc<Instance>,

    /// The physical device, which contains useful property and limit data.
    pub physical_device: PhysicalDevice,

    /// The physical execution queues which all work will be submitted to.
    pub(crate) queues: Box<[Queue]>,

    /// Describes the features of the device which relate to ray query, if available.
    pub ray_query_features: Option<PhysicalDeviceRayQueryFeatures>,

    pub(crate) ray_tracing_pipeline_ext: Option<khr::RayTracingPipeline>,

    /// Describes the features of the device which relate to ray tracing, if available.
    pub ray_tracing_pipeline_features: Option<PhysicalDeviceRayTracingPipelineFeatures>,

    /// Describes the properties of the device which relate to ray tracing, if available.
    pub ray_tracing_pipeline_properties: Option<PhysicalDeviceRayTracePipelineProperties>,

    pub(super) surface_ext: Option<khr::Surface>,
    pub(super) swapchain_ext: Option<khr::Swapchain>,

    /// Describes the features of the device which are part of the Vulkan 1.0 base feature set.
    pub vulkan_1_0_features: PhysicalDeviceVulkan10Features,

    /// Describes the features of the device which are part of the Vulkan 1.1 base feature set.
    pub vulkan_1_1_features: PhysicalDeviceVulkan11Features,

    /// Describes the properties of the device which are part of the Vulkan 1.1 base feature set.
    pub vulkan_1_1_properties: PhysicalDeviceVulkan11Properties,

    /// Describes the features of the device which are part of the Vulkan 1.2 base feature set.
    pub vulkan_1_2_features: PhysicalDeviceVulkan12Features,

    /// Describes the properties of the device which are part of the Vulkan 1.2 base feature set.
    pub vulkan_1_2_properties: PhysicalDeviceVulkan12Properties,
}

impl Device {
    /// Constructs a new device using the given configuration.
    pub fn new(cfg: impl Into<DriverConfig>) -> Result<Self, DriverError> {
        let cfg = cfg.into();

        trace!("new {:?}", cfg);

        let instance = Arc::new(Instance::new(cfg.debug, empty())?);
        let physical_device = Instance::physical_devices(&instance)?
            .collect::<Vec<_>>()
            .into_iter()
            // If there are multiple devices with the same score, `max_by_key` would choose the last,
            // and we want to preserve the order of devices from `enumerate_physical_devices`.
            .rev()
            .max_by_key(PhysicalDevice::score_device_type)
            .ok_or(DriverError::Unsupported)?;

        Device::create(&instance, physical_device, cfg)
    }

    pub(super) fn create(
        instance: &Arc<Instance>,
        physical_device: PhysicalDevice,
        cfg: DriverConfig,
    ) -> Result<Self, DriverError> {
        let instance = Arc::clone(instance);
        let InstanceFnV1_1 {
            get_physical_device_features2,
            get_physical_device_properties2,
            ..
        } = instance.fp_v1_1();

        let accel_struct_ext;
        let ray_query_ext;
        let ray_tracing_pipeline_ext;
        let swapchain_ext;

        unsafe {
            let ext_properties = instance
                .enumerate_device_extension_properties(*physical_device)
                .map_err(|err| {
                    error!("{err}");

                    DriverError::Unsupported
                })?;

            for prop in &ext_properties {
                debug!(
                    "extension {:?} v{}",
                    CStr::from_ptr(prop.extension_name.as_ptr()),
                    prop.spec_version
                );
            }

            let supported_exts = ext_properties
                .iter()
                .map(|prop| CStr::from_ptr(prop.extension_name.as_ptr() as *const _))
                .collect::<HashSet<_>>();

            accel_struct_ext = supported_exts.contains(vk::KhrAccelerationStructureFn::name())
                && supported_exts.contains(vk::KhrDeferredHostOperationsFn::name());
            ray_query_ext = supported_exts.contains(vk::KhrRayQueryFn::name());
            ray_tracing_pipeline_ext = supported_exts.contains(vk::KhrRayTracingPipelineFn::name());
            swapchain_ext = supported_exts.contains(vk::KhrSwapchainFn::name());
        };

        let mut enabled_ext_names = vec![];

        if accel_struct_ext {
            enabled_ext_names.push(vk::KhrAccelerationStructureFn::name().as_ptr());
            enabled_ext_names.push(vk::KhrDeferredHostOperationsFn::name().as_ptr());
        }

        if ray_query_ext {
            enabled_ext_names.push(vk::KhrRayQueryFn::name().as_ptr());
        }

        if ray_tracing_pipeline_ext {
            enabled_ext_names.push(vk::KhrRayTracingPipelineFn::name().as_ptr());
        }

        if swapchain_ext {
            enabled_ext_names.push(vk::KhrSwapchainFn::name().as_ptr());
        }

        let queue_family = PhysicalDevice::queue_families(&physical_device).find(|qf| {
            qf.props.queue_flags.contains(
                vk::QueueFlags::COMPUTE & vk::QueueFlags::GRAPHICS & vk::QueueFlags::TRANSFER,
            )
        });

        let queue_family = if let Some(queue_family) = queue_family {
            queue_family
        } else {
            warn!("no suitable queue family found");

            return Err(DriverError::Unsupported);
        };

        let priorities = repeat(1.0)
            .take(
                cfg.desired_queue_count
                    .clamp(1, queue_family.props.queue_count as _),
            )
            .collect::<Box<_>>();
        let mut queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family.idx)
            .queue_priorities(&priorities)
            .build();
        queue_info.queue_count = priorities.len() as _;

        let mut vulkan_1_1_features = vk::PhysicalDeviceVulkan11Features::builder();
        let mut vulkan_1_2_features = vk::PhysicalDeviceVulkan12Features::builder();

        let mut accel_struct_features =
            accel_struct_ext.then(vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default);
        let mut ray_query_features =
            ray_query_ext.then(vk::PhysicalDeviceRayQueryFeaturesKHR::default);
        let mut ray_tracing_pipeline_features =
            ray_tracing_pipeline_ext.then(vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default);

        unsafe {
            let mut features = vk::PhysicalDeviceFeatures2::builder()
                .push_next(&mut vulkan_1_1_features)
                .push_next(&mut vulkan_1_2_features);

            if let Some(ext) = &mut accel_struct_features {
                features = features.push_next(ext);
            }

            if let Some(ext) = &mut ray_query_features {
                features = features.push_next(ext);
            }

            if let Some(ext) = &mut ray_tracing_pipeline_features {
                features = features.push_next(ext);
            }

            let mut features = features.build();

            get_physical_device_features2(*physical_device, &mut features);

            if features.features.multi_draw_indirect != vk::TRUE {
                warn!("device does not support multi draw indirect");

                return Err(DriverError::Unsupported);
            }

            if features.features.sampler_anisotropy != vk::TRUE {
                warn!("device does not support sampler anisotropy");

                return Err(DriverError::Unsupported);
            }

            if vulkan_1_2_features.imageless_framebuffer != vk::TRUE {
                warn!("device does not support imageless framebuffer");

                return Err(DriverError::Unsupported);
            }

            #[cfg(not(target_os = "macos"))]
            if vulkan_1_2_features.separate_depth_stencil_layouts != vk::TRUE {
                warn!("device does not support separate depth stencil layouts");

                return Err(DriverError::Unsupported);
            }

            let mut accel_struct_properties =
                vk::PhysicalDeviceAccelerationStructurePropertiesKHR::default();
            let mut ray_tracing_pipeline_properties =
                vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();
            let mut depth_stencil_resolve_properties =
                vk::PhysicalDeviceDepthStencilResolveProperties::default();
            let mut vulkan_1_1_properties = vk::PhysicalDeviceVulkan11Properties::default();
            let mut vulkan_1_2_properties = vk::PhysicalDeviceVulkan12Properties::default();
            let mut physical_properties = vk::PhysicalDeviceProperties2::builder()
                .push_next(&mut depth_stencil_resolve_properties)
                .push_next(&mut vulkan_1_1_properties)
                .push_next(&mut vulkan_1_2_properties);

            if accel_struct_ext {
                physical_properties = physical_properties
                    .push_next(&mut accel_struct_properties)
                    .push_next(&mut ray_tracing_pipeline_properties);
            }

            let mut physical_properties = physical_properties.build();
            get_physical_device_properties2(*physical_device, &mut physical_properties);

            let depth_stencil_resolve_properties = depth_stencil_resolve_properties.into();

            let accel_struct_properties = accel_struct_ext.then(|| accel_struct_properties.into());
            let ray_query_features = ray_query_ext.then(|| ray_query_features.unwrap().into());
            let ray_tracing_pipeline_features =
                ray_tracing_pipeline_ext.then(|| ray_tracing_pipeline_features.unwrap().into());
            let ray_tracing_pipeline_properties =
                ray_tracing_pipeline_ext.then(|| ray_tracing_pipeline_properties.into());

            let queue_infos = [queue_info];
            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_infos)
                .enabled_extension_names(&enabled_ext_names)
                .push_next(&mut features);
            let device = instance
                .create_device(*physical_device, &device_create_info, None)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
            let allocator = Allocator::new(&AllocatorCreateDesc {
                instance: (**instance).clone(),
                device: device.clone(),
                physical_device: *physical_device,
                debug_settings: AllocatorDebugSettings {
                    log_leaks_on_shutdown: cfg.debug,
                    log_memory_information: cfg.debug,
                    log_allocations: cfg.debug,
                    ..Default::default()
                },
                buffer_device_address: true,
            })
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;
            let queues = repeat(queue_family)
                .take(priorities.len())
                .enumerate()
                .map(|(queue_index, queue_family)| Queue {
                    queue: device.get_device_queue(queue_family.idx, queue_index as _),
                    family: queue_family,
                })
                .collect();

            let immutable_samplers = Self::create_immutable_samplers(&device)?;

            let (surface_ext, swapchain_ext) = if swapchain_ext {
                (
                    Some(khr::Surface::new(&instance.entry, &instance)),
                    Some(khr::Swapchain::new(&instance, &device)),
                )
            } else {
                (None, None)
            };
            let accel_struct_ext =
                accel_struct_ext.then(|| khr::AccelerationStructure::new(&instance, &device));
            let ray_tracing_pipeline_ext =
                ray_tracing_pipeline_ext.then(|| khr::RayTracingPipeline::new(&instance, &device));

            let vulkan_1_0_features = features.features.into();
            let vulkan_1_1_features = vulkan_1_1_features.build().into();
            let vulkan_1_1_properties = vulkan_1_1_properties.into();
            let vulkan_1_2_features: PhysicalDeviceVulkan12Features =
                vulkan_1_2_features.build().into();
            let vulkan_1_2_properties = vulkan_1_2_properties.into();

            Ok(
                #[allow(deprecated)]
                Self {
                    accel_struct_ext,
                    accel_struct_properties,
                    allocator: Some(Mutex::new(allocator)),
                    depth_stencil_resolve_properties,
                    device,
                    immutable_samplers,
                    instance,
                    physical_device,
                    queues,
                    ray_query_features,
                    ray_tracing_pipeline_ext,
                    ray_tracing_pipeline_features,
                    ray_tracing_pipeline_properties,
                    surface_ext,
                    swapchain_ext,
                    vulkan_1_0_features,
                    vulkan_1_1_features,
                    vulkan_1_1_properties,
                    vulkan_1_2_features,
                    vulkan_1_2_properties,
                },
            )
        }
    }

    fn create_immutable_samplers(
        device: &ash::Device,
    ) -> Result<HashMap<SamplerDesc, vk::Sampler>, DriverError> {
        let texel_filters = [vk::Filter::LINEAR, vk::Filter::NEAREST];
        let mipmap_modes = [
            vk::SamplerMipmapMode::LINEAR,
            vk::SamplerMipmapMode::NEAREST,
        ];
        let address_modes = [
            vk::SamplerAddressMode::CLAMP_TO_BORDER,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::MIRRORED_REPEAT,
            vk::SamplerAddressMode::REPEAT,
        ];

        let mut res = HashMap::new();

        for texel_filter in texel_filters {
            for mipmap_mode in mipmap_modes {
                for address_modes in address_modes {
                    let anisotropy_enable = texel_filter == vk::Filter::LINEAR;

                    res.insert(
                        SamplerDesc {
                            texel_filter,
                            mipmap_mode,
                            address_modes,
                        },
                        unsafe {
                            let mut info = vk::SamplerCreateInfo::builder()
                                .mag_filter(texel_filter)
                                .min_filter(texel_filter)
                                .mipmap_mode(mipmap_mode)
                                .address_mode_u(address_modes)
                                .address_mode_v(address_modes)
                                .address_mode_w(address_modes)
                                .max_lod(vk::LOD_CLAMP_NONE)
                                .anisotropy_enable(anisotropy_enable);

                            if anisotropy_enable {
                                info = info.max_anisotropy(16.0);
                            }

                            device.create_sampler(&info, None)
                        }
                        .map_err(|err| {
                            warn!("{err}");

                            DriverError::Unsupported
                        })?,
                    );
                }
            }
        }

        Ok(res)
    }

    /// Lists the physical device's format capabilities.
    pub fn get_format_properties(this: &Self, format: vk::Format) -> vk::FormatProperties {
        unsafe {
            this.instance
                .get_physical_device_format_properties(*this.physical_device, format)
        }
    }

    /// Lists the physical device's image format capabilities.
    pub fn get_image_format_properties(
        this: &Self,
        format: vk::Format,
        ty: vk::ImageType,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        flags: vk::ImageCreateFlags,
    ) -> Result<vk::ImageFormatProperties, DriverError> {
        unsafe {
            match this.instance.get_physical_device_image_format_properties(
                *this.physical_device,
                format,
                ty,
                tiling,
                usage,
                flags,
            ) {
                Ok(properties) => Ok(properties),
                Err(err) if err == vk::Result::ERROR_FORMAT_NOT_SUPPORTED => {
                    error!("Format not supported");

                    Err(DriverError::Unsupported)
                }
                _ => Err(DriverError::OutOfMemory),
            }
        }
    }

    pub(super) fn immutable_sampler(this: &Self, info: SamplerDesc) -> vk::Sampler {
        this.immutable_samplers
            .get(&info)
            .copied()
            .unwrap_or_else(|| unimplemented!("{:?}", info))
    }

    /// Returns the count of available queues created by the device.
    ///
    /// See [`DriverConfig.desired_queue_count`].
    pub fn queue_count(this: &Self) -> usize {
        this.queues.len()
    }

    pub(super) fn surface_formats(
        this: &Self,
        surface: &Surface,
    ) -> Result<Vec<vk::SurfaceFormatKHR>, DriverError> {
        unsafe {
            this.surface_ext
                .as_ref()
                .unwrap()
                .get_physical_device_surface_formats(*this.physical_device, **surface)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })
        }
    }

    pub(crate) fn wait_for_fence(this: &Self, fence: &vk::Fence) -> Result<(), DriverError> {
        use std::slice::from_ref;

        Device::wait_for_fences(this, from_ref(fence))
    }

    pub(crate) fn wait_for_fences(this: &Self, fences: &[vk::Fence]) -> Result<(), DriverError> {
        unsafe {
            match this.device.wait_for_fences(fences, true, 100) {
                Ok(_) => return Ok(()),
                Err(err) if err == vk::Result::ERROR_DEVICE_LOST => {
                    error!("Device lost");

                    return Err(DriverError::InvalidData);
                }
                Err(err) if err == vk::Result::TIMEOUT => {
                    trace!("waiting...");
                }
                _ => return Err(DriverError::OutOfMemory),
            }

            let started = Instant::now();

            match this.device.wait_for_fences(fences, true, u64::MAX) {
                Ok(_) => (),
                Err(err) if err == vk::Result::ERROR_DEVICE_LOST => {
                    error!("Device lost");

                    return Err(DriverError::InvalidData);
                }
                _ => return Err(DriverError::OutOfMemory),
            }

            let elapsed = Instant::now() - started;
            let elapsed_millis = elapsed.as_millis();

            if elapsed_millis > 0 {
                warn!("waited for {} ms", elapsed_millis);
            }
        }

        Ok(())
    }
}

impl Debug for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Device")
    }
}

impl Deref for Device {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if panicking() {
            // When panicking we don't want the GPU allocator to complain about leaks
            forget(self.allocator.take().unwrap());

            return;
        }

        // trace!("drop");

        let res = unsafe { self.device.device_wait_idle() };

        if let Err(err) = res {
            warn!("device_wait_idle() failed: {err}");
        }

        self.allocator.take().unwrap();

        for (_, sampler) in self.immutable_samplers.drain() {
            unsafe {
                self.device.destroy_sampler(sampler, None);
            }
        }

        unsafe {
            self.device.destroy_device(None);
        }
    }
}
