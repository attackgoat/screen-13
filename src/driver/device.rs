use {
    super::{
        DriverConfig, DriverError, Instance, PhysicalDevice,
        PhysicalDeviceRayTracePipelineProperties, QueueFamily, SamplerDesc, Surface,
    },
    archery::{SharedPointer, SharedPointerKind},
    ash::{extensions::khr, vk},
    gpu_allocator::{
        vulkan::{Allocator, AllocatorCreateDesc},
        AllocatorDebugSettings,
    },
    log::{debug, info, trace, warn},
    parking_lot::Mutex,
    std::{
        collections::{HashMap, HashSet},
        ffi::CStr,
        fmt::{Debug, Formatter},
        iter::empty,
        mem::forget,
        ops::Deref,
        os::raw::c_char,
        thread::panicking,
        time::Instant,
    },
};

pub struct Device<P>
where
    P: SharedPointerKind,
{
    pub accel_struct_ext: Option<khr::AccelerationStructure>,
    pub(super) allocator: Option<Mutex<Allocator>>,
    device: ash::Device,
    immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,
    pub instance: SharedPointer<Instance, P>, // TODO: Need shared?
    pub physical_device: PhysicalDevice,
    pub queue: Queue,
    pub ray_tracing_pipeline_ext: Option<khr::RayTracingPipeline>,
    pub ray_tracing_pipeline_properties: Option<PhysicalDeviceRayTracePipelineProperties>,
    pub surface_ext: Option<khr::Surface>,
    pub swapchain_ext: Option<khr::Swapchain>,
}

impl<P> Device<P>
where
    P: SharedPointerKind,
{
    pub fn new(cfg: DriverConfig) -> Result<Self, DriverError> {
        trace!("new {:?}", cfg);

        let instance = SharedPointer::new(Instance::new(cfg.debug, empty())?);
        let physical_device = Instance::physical_devices(&instance)?
            .filter(|physical_device| {
                if cfg.ray_tracing && !PhysicalDevice::has_ray_tracing_support(physical_device) {
                    info!("{:?} lacks ray tracing support", unsafe {
                        CStr::from_ptr(physical_device.props.device_name.as_ptr() as *const c_char)
                    });

                    return false;
                }

                // TODO: Check vkGetPhysicalDeviceFeatures for samplerAnisotropy (it should exist, but to be sure)

                true
            })
            .collect::<Vec<_>>()
            .into_iter()
            // If there are multiple devices with the same score, `max_by_key` would choose the last,
            // and we want to preserve the order of devices from `enumerate_physical_devices`.
            .rev()
            .max_by_key(PhysicalDevice::score_device_type)
            .ok_or(DriverError::Unsupported)?;

        Device::create(&instance, physical_device, cfg)
    }

    pub fn create(
        instance: &SharedPointer<Instance, P>,
        physical_device: PhysicalDevice,
        cfg: DriverConfig,
    ) -> Result<Self, DriverError> {
        let instance = SharedPointer::clone(instance);
        let fp_v1_1 = instance.fp_v1_1();
        let get_physical_device_features2 = fp_v1_1.get_physical_device_features2;
        let get_physical_device_properties2 = fp_v1_1.get_physical_device_properties2;

        let features = cfg.features();
        let device_extension_names = features.extension_names();

        unsafe {
            let extension_properties = instance
                .enumerate_device_extension_properties(*physical_device)
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;

            for ext in &extension_properties {
                debug!(
                    "extension {:?} v{}",
                    CStr::from_ptr(ext.extension_name.as_ptr()),
                    ext.spec_version
                );
            }

            let supported_extensions: HashSet<String> = extension_properties
                .iter()
                .map(|ext| {
                    CStr::from_ptr(ext.extension_name.as_ptr() as *const c_char)
                        .to_string_lossy()
                        .as_ref()
                        .to_owned()
                })
                .collect();

            for &ext in &device_extension_names {
                let ext = CStr::from_ptr(ext).to_string_lossy();
                if !supported_extensions.contains(ext.as_ref()) {
                    warn!("unsupported: {}", ext);

                    return Err(DriverError::Unsupported);
                }
            }
        };

        let priorities = [1.0];
        let queue = PhysicalDevice::queue_families(&physical_device).find(|qf| {
            qf.props.queue_flags.contains(
                vk::QueueFlags::COMPUTE & vk::QueueFlags::GRAPHICS & vk::QueueFlags::TRANSFER,
            )
        });

        let queue = if let Some(queue) = queue {
            queue
        } else {
            warn!("no suitable presentation queue found");

            return Err(DriverError::Unsupported);
        };

        let queue_info = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue.idx)
            .queue_priorities(&priorities)
            .build()];

        let mut imageless_framebuffer_features =
            vk::PhysicalDeviceImagelessFramebufferFeatures::builder();
        let mut buffer_device_address_features =
            vk::PhysicalDeviceBufferDeviceAddressFeatures::builder();

        #[cfg(not(target_os = "macos"))]
        let mut separate_depth_stencil_layouts_features =
            vk::PhysicalDeviceSeparateDepthStencilLayoutsFeatures::builder();

        let mut acceleration_struct_features = if features.ray_tracing {
            Some(ash::vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default())
        } else {
            None
        };

        let mut ray_tracing_pipeline_features = if features.ray_tracing {
            Some(ash::vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default())
        } else {
            None
        };

        unsafe {
            let mut features2 = vk::PhysicalDeviceFeatures2::builder()
                .push_next(&mut buffer_device_address_features)
                .push_next(&mut imageless_framebuffer_features);

            #[cfg(not(target_os = "macos"))]
            {
                features2 = features2.push_next(&mut separate_depth_stencil_layouts_features);
            }

            if features.ray_tracing {
                features2 = features2
                    .push_next(acceleration_struct_features.as_mut().unwrap())
                    .push_next(ray_tracing_pipeline_features.as_mut().unwrap());
            }

            let mut features2 = features2.build();

            get_physical_device_features2(*physical_device, &mut features2);

            if features2.features.multi_draw_indirect != vk::TRUE {
                warn!("device does not support multi draw indirect");

                return Err(DriverError::Unsupported);
            }

            if features2.features.sampler_anisotropy != vk::TRUE {
                warn!("device does not support sampler anisotropy");

                return Err(DriverError::Unsupported);
            }

            if imageless_framebuffer_features.imageless_framebuffer != vk::TRUE {
                warn!("device does not support imageless framebuffer");

                return Err(DriverError::Unsupported);
            }

            #[cfg(not(target_os = "macos"))]
            if separate_depth_stencil_layouts_features.separate_depth_stencil_layouts != vk::TRUE {
                warn!("device does not support separate depth stencil layouts");

                return Err(DriverError::Unsupported);
            }

            // debug!("{:#?}", &features2.features);
            // debug!("{:#?}", &scalar_block);
            // debug!("{:#?}", &descriptor_indexing);
            // debug!("{:#?}", &imageless_framebuffer);
            // debug!("{:#?}", &shader_float16_int8);
            // debug!("{:#?}", &vulkan_memory_model);
            // debug!("{:#?}", &get_buffer_device_address_features);

            // assert!(scalar_block.scalar_block_layout != 0);

            //assert!(descriptor_indexing.shader_uniform_texel_buffer_array_dynamic_indexing != 0);
            //assert!(descriptor_indexing.shader_storage_texel_buffer_array_dynamic_indexing != 0);
            //assert!(descriptor_indexing.shader_uniform_buffer_array_non_uniform_indexing != 0);
            //assert!(descriptor_indexing.shader_sampled_image_array_non_uniform_indexing != 0);
            //assert!(descriptor_indexing.shader_storage_buffer_array_non_uniform_indexing != 0);
            //assert!(descriptor_indexing.shader_storage_image_array_non_uniform_indexing != 0);
            // assert!(
            //     descriptor_indexing.shader_uniform_texel_buffer_array_non_uniform_indexing != 0
            // );
            // assert!(
            //     descriptor_indexing.shader_storage_texel_buffer_array_non_uniform_indexing != 0
            // );
            // assert!(descriptor_indexing.descriptor_binding_sampled_image_update_after_bind != 0);
            // assert!(descriptor_indexing.descriptor_binding_update_unused_while_pending != 0);
            // assert!(descriptor_indexing.descriptor_binding_partially_bound != 0);
            // assert!(descriptor_indexing.descriptor_binding_variable_descriptor_count != 0);
            // assert!(descriptor_indexing.runtime_descriptor_array != 0);

            // assert!(shader_float16_int8.shader_int8 != 0);

            //assert!(vulkan_memory_model.vulkan_memory_model != 0);

            if features.ray_tracing {
                if buffer_device_address_features.buffer_device_address != vk::TRUE {
                    warn!("device does not support buffer device address");

                    return Err(DriverError::Unsupported);
                }

                let acceleration_struct_features = acceleration_struct_features.as_ref().unwrap();

                if acceleration_struct_features.acceleration_structure != vk::TRUE {
                    warn!("device does not support acceleration structure");

                    return Err(DriverError::Unsupported);
                }

                if acceleration_struct_features
                    .descriptor_binding_acceleration_structure_update_after_bind
                    != vk::TRUE
                {
                    warn!("device does not support descriptor binding acceleration structure update after bind");

                    return Err(DriverError::Unsupported);
                }

                let ray_tracing_pipeline_features = ray_tracing_pipeline_features.as_ref().unwrap();

                if ray_tracing_pipeline_features.ray_tracing_pipeline != vk::TRUE {
                    warn!("device does not support ray tracing pipeline");

                    return Err(DriverError::Unsupported);
                }

                if ray_tracing_pipeline_features.ray_tracing_pipeline_trace_rays_indirect
                    != vk::TRUE
                {
                    warn!("device does not support ray tracing pipeline trace rays indirect");

                    return Err(DriverError::Unsupported);
                }
            }

            let mut ray_tracing_pipeline_properties =
                vk::PhysicalDeviceRayTracingPipelinePropertiesKHR::default();

            let mut physical_properties = vk::PhysicalDeviceProperties2::builder();

            if features.ray_tracing {
                physical_properties =
                    physical_properties.push_next(&mut ray_tracing_pipeline_properties);
            }

            let mut physical_properties = physical_properties.build();
            get_physical_device_properties2(*physical_device, &mut physical_properties);

            let ray_tracing_pipeline_properties = if features.ray_tracing {
                Some(ray_tracing_pipeline_properties.into())
            } else {
                None
            };

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names)
                .push_next(&mut features2);
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
            let queue = Queue {
                queue: device.get_device_queue(queue.idx, 0),
                family: queue,
            };

            let immutable_samplers = Self::create_immutable_samplers(&device)?;

            let (surface_ext, swapchain_ext) = if cfg.presentation {
                (
                    Some(khr::Surface::new(&instance.entry, &instance)),
                    Some(khr::Swapchain::new(&instance, &device)),
                )
            } else {
                (None, None)
            };

            let (accel_struct_ext, ray_tracing_pipeline_ext) = if cfg.ray_tracing {
                (
                    Some(khr::AccelerationStructure::new(&instance, &device)),
                    Some(khr::RayTracingPipeline::new(&instance, &device)),
                )
            } else {
                (None, None)
            };

            Ok(Self {
                accel_struct_ext,
                allocator: Some(Mutex::new(allocator)),
                device,
                immutable_samplers,
                instance,
                physical_device,
                queue,
                ray_tracing_pipeline_ext,
                ray_tracing_pipeline_properties,
                surface_ext,
                swapchain_ext,
            })
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

    pub fn immutable_sampler(this: &Self, info: SamplerDesc) -> vk::Sampler {
        this.immutable_samplers
            .get(&info)
            .copied()
            .unwrap_or_else(|| unimplemented!("{:?}", info))
    }

    pub fn surface_formats(
        this: &Self,
        surface: &Surface<impl SharedPointerKind>,
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

    pub fn wait_for_fence(this: &Self, fence: &vk::Fence) -> Result<(), DriverError> {
        use std::slice::from_ref;

        Device::wait_for_fences(this, from_ref(fence))
    }

    pub fn wait_for_fences(this: &Self, fences: &[vk::Fence]) -> Result<(), DriverError> {
        unsafe {
            match this.device.wait_for_fences(fences, true, 100) {
                Ok(_) => return Ok(()),
                Err(err) if err == vk::Result::ERROR_DEVICE_LOST => (),
                Err(err) if err == vk::Result::TIMEOUT => {
                    trace!("waiting...");
                }
                _ => return Err(DriverError::OutOfMemory),
            }

            let started = Instant::now();

            match this.device.wait_for_fences(fences, true, u64::MAX) {
                Ok(_) => (),
                Err(err) if err == vk::Result::ERROR_DEVICE_LOST => (),
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

impl<P> Debug for Device<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Device")
    }
}

impl<P> Deref for Device<P>
where
    P: SharedPointerKind,
{
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl<P> Drop for Device<P>
where
    P: SharedPointerKind,
{
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

pub struct FeatureFlags {
    pub presentation: bool,
    pub ray_tracing: bool,
}

impl FeatureFlags {
    pub fn extension_names(&self) -> Vec<*const i8> {
        let mut res = vec![];

        #[cfg(target_os = "macos")]
        {
            res.extend([
                vk::KhrBufferDeviceAddressFn::name(),
                vk::KhrCreateRenderpass2Fn::name(),
                vk::KhrImagelessFramebufferFn::name(),
                vk::KhrImageFormatListFn::name(),
                vk::KhrSeparateDepthStencilLayoutsFn::name(),
            ]);
        }

        if self.presentation {
            res.push(khr::Swapchain::name());
        }

        if self.ray_tracing {
            res.extend(
                [
                    vk::KhrAccelerationStructureFn::name(),
                    vk::KhrDeferredHostOperationsFn::name(),
                    vk::KhrRayTracingPipelineFn::name(),
                ]
                .iter(),
            );
        }

        res.iter().map(|name| name.as_ptr()).collect()
    }
}

pub struct Queue {
    queue: vk::Queue,
    pub family: QueueFamily,
}

impl Deref for Queue {
    type Target = vk::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}
