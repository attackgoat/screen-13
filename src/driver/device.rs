use {
    super::{
        DriverConfig, DriverError, Instance, PhysicalDevice, QueueFamily, SamplerDesc, Surface,
    },
    crate::ptr::Shared,
    archery::SharedPointerKind,
    ash::{extensions::khr, vk},
    bitflags::bitflags,
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
    pub(super) allocator: Option<Mutex<Allocator>>,
    device: ash::Device,
    immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,
    pub instance: Shared<Instance, P>, // TODO: Need shared?
    pub physical_device: PhysicalDevice,
    pub queue: Queue,

    // Vulkan extensions
    pub accel_struct_ext: khr::AccelerationStructure,
    pub ray_trace_pipeline_ext: khr::RayTracingPipeline,
    pub ray_trace_pipeline_properties: vk::PhysicalDeviceRayTracingPipelinePropertiesKHR,
    pub surface_ext: khr::Surface,
    pub swapchain_ext: khr::Swapchain,
}

impl<P> Device<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        instance: &Shared<Instance, P>,
        physical_device: PhysicalDevice,
        cfg: DriverConfig,
    ) -> Result<Self, DriverError> {
        let instance = Shared::clone(instance);
        let features = cfg.features();
        let device_extension_names = features.extension_names();

        unsafe {
            let extension_properties = instance
                .enumerate_device_extension_properties(*physical_device)
                .map_err(|_| DriverError::Unsupported)?;

            #[cfg(debug_assertions)]
            debug!("Extension properties:\n{:#?}", &extension_properties);

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
                    #[cfg(debug_assertions)]
                    warn!("Unsupported: {}", ext);

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
            warn!("No suitable presentation queue found");

            return Err(DriverError::Unsupported);
        };

        let queue_info = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue.idx)
            .queue_priorities(&priorities)
            .build()];

        let mut scalar_block = vk::PhysicalDeviceScalarBlockLayoutFeaturesEXT::default();
        let mut descriptor_indexing = vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::default();
        let mut imageless_framebuffer =
            vk::PhysicalDeviceImagelessFramebufferFeaturesKHR::default();
        let mut shader_float16_int8 = vk::PhysicalDeviceShaderFloat16Int8Features::default();
        let mut vulkan_memory_model = vk::PhysicalDeviceVulkanMemoryModelFeaturesKHR::default();
        let mut get_buffer_device_address_features =
            ash::vk::PhysicalDeviceBufferDeviceAddressFeatures::default();

        let mut acceleration_struct_features = if features.contains(FeatureFlags::RAY_TRACING) {
            Some(ash::vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default())
        } else {
            None
        };

        let mut ray_tracing_pipeline_features = if features.contains(FeatureFlags::RAY_TRACING) {
            Some(ash::vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default())
        } else {
            None
        };

        unsafe {
            let mut features2 = vk::PhysicalDeviceFeatures2::builder()
                .push_next(&mut scalar_block)
                .push_next(&mut descriptor_indexing)
                .push_next(&mut imageless_framebuffer)
                .push_next(&mut shader_float16_int8)
                .push_next(&mut vulkan_memory_model)
                .push_next(&mut get_buffer_device_address_features);

            if features.contains(FeatureFlags::RAY_TRACING) {
                features2 = features2
                    .push_next(acceleration_struct_features.as_mut().unwrap())
                    .push_next(ray_tracing_pipeline_features.as_mut().unwrap());
            }

            let mut features2 = features2.build();

            instance
                .fp_v1_1()
                .get_physical_device_features2(*physical_device, &mut features2);

            debug!("{:#?}", &scalar_block);
            debug!("{:#?}", &descriptor_indexing);
            debug!("{:#?}", &imageless_framebuffer);
            debug!("{:#?}", &shader_float16_int8);
            debug!("{:#?}", &vulkan_memory_model);
            debug!("{:#?}", &get_buffer_device_address_features);

            assert!(scalar_block.scalar_block_layout != 0);

            assert!(descriptor_indexing.shader_uniform_texel_buffer_array_dynamic_indexing != 0);
            assert!(descriptor_indexing.shader_storage_texel_buffer_array_dynamic_indexing != 0);
            assert!(descriptor_indexing.shader_uniform_buffer_array_non_uniform_indexing != 0);
            assert!(descriptor_indexing.shader_sampled_image_array_non_uniform_indexing != 0);
            assert!(descriptor_indexing.shader_storage_buffer_array_non_uniform_indexing != 0);
            assert!(descriptor_indexing.shader_storage_image_array_non_uniform_indexing != 0);
            assert!(
                descriptor_indexing.shader_uniform_texel_buffer_array_non_uniform_indexing != 0
            );
            assert!(
                descriptor_indexing.shader_storage_texel_buffer_array_non_uniform_indexing != 0
            );
            assert!(descriptor_indexing.descriptor_binding_sampled_image_update_after_bind != 0);
            assert!(descriptor_indexing.descriptor_binding_update_unused_while_pending != 0);
            assert!(descriptor_indexing.descriptor_binding_partially_bound != 0);
            assert!(descriptor_indexing.descriptor_binding_variable_descriptor_count != 0);
            assert!(descriptor_indexing.runtime_descriptor_array != 0);

            assert!(imageless_framebuffer.imageless_framebuffer != 0);

            assert!(shader_float16_int8.shader_int8 != 0);

            assert!(vulkan_memory_model.vulkan_memory_model != 0);

            if features.contains(FeatureFlags::RAY_TRACING) {
                assert!(
                    acceleration_struct_features
                        .as_ref()
                        .unwrap()
                        .acceleration_structure
                        != 0
                );
                assert!(
                    acceleration_struct_features
                        .as_ref()
                        .unwrap()
                        .descriptor_binding_acceleration_structure_update_after_bind
                        != 0
                );

                assert!(
                    ray_tracing_pipeline_features
                        .as_ref()
                        .unwrap()
                        .ray_tracing_pipeline
                        != 0
                );
                assert!(
                    ray_tracing_pipeline_features
                        .as_ref()
                        .unwrap()
                        .ray_tracing_pipeline_trace_rays_indirect
                        != 0
                );
            }

            assert!(get_buffer_device_address_features.buffer_device_address != 0);

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names)
                .push_next(&mut features2)
                .build();
            let device = instance
                .create_device(*physical_device, &device_create_info, None)
                .map_err(|_| DriverError::Unsupported)?;
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
            .map_err(|_| DriverError::Unsupported)?;
            let queue = Queue {
                queue: device.get_device_queue(queue.idx, 0),
                family: queue,
            };

            let immutable_samplers = Self::create_immutable_samplers(&device)?;

            // Get extensions
            let accel_struct_ext = khr::AccelerationStructure::new(&instance, &device);
            let ray_trace_pipeline_ext = khr::RayTracingPipeline::new(&instance, &device);
            let ray_trace_pipeline_properties =
                khr::RayTracingPipeline::get_properties(&instance, *physical_device);
            let surface_ext = khr::Surface::new(&instance.entry, &instance);
            let swapchain_ext = khr::Swapchain::new(&instance, &device);

            Ok(Self {
                allocator: Some(Mutex::new(allocator)),
                device,
                immutable_samplers,
                instance,
                physical_device,
                queue,

                // Vulkan extensions
                accel_struct_ext,
                ray_trace_pipeline_ext,
                ray_trace_pipeline_properties,
                surface_ext,
                swapchain_ext,
            })
        }
    }

    fn create_immutable_samplers(
        device: &ash::Device,
    ) -> Result<HashMap<SamplerDesc, vk::Sampler>, DriverError> {
        let texel_filters = [vk::Filter::NEAREST, vk::Filter::LINEAR];
        let mipmap_modes = [
            vk::SamplerMipmapMode::NEAREST,
            vk::SamplerMipmapMode::LINEAR,
        ];
        let address_modes = [
            vk::SamplerAddressMode::CLAMP_TO_BORDER,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerAddressMode::MIRRORED_REPEAT,
            vk::SamplerAddressMode::REPEAT,
        ];

        let mut res = HashMap::new();

        for &texel_filter in &texel_filters {
            for &mipmap_mode in &mipmap_modes {
                for &address_modes in &address_modes {
                    let anisotropy_enable = texel_filter == vk::Filter::LINEAR;

                    res.insert(
                        SamplerDesc {
                            texel_filter,
                            mipmap_mode,
                            address_modes,
                        },
                        unsafe {
                            device.create_sampler(
                                &vk::SamplerCreateInfo::builder()
                                    .mag_filter(texel_filter)
                                    .min_filter(texel_filter)
                                    .mipmap_mode(mipmap_mode)
                                    .address_mode_u(address_modes)
                                    .address_mode_v(address_modes)
                                    .address_mode_w(address_modes)
                                    .max_lod(vk::LOD_CLAMP_NONE)
                                    .max_anisotropy(16.0)
                                    .anisotropy_enable(anisotropy_enable)
                                    .build(),
                                None,
                            )
                        }
                        .map_err(|_| DriverError::Unsupported)?,
                    );
                }
            }
        }

        Ok(res)
    }

    pub(crate) fn immutable_sampler(this: &Self, info: SamplerDesc) -> Option<vk::Sampler> {
        this.immutable_samplers.get(&info).copied()
    }

    pub fn surface_formats(
        this: &Self,
        surface: &Surface<impl SharedPointerKind>,
    ) -> Result<Vec<vk::SurfaceFormatKHR>, DriverError> {
        unsafe {
            this.surface_ext
                .get_physical_device_surface_formats(*this.physical_device, **surface)
                .map_err(|_| DriverError::Unsupported)
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

                //panic!();
            } else {
                trace!("...done")
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

        if res.is_err() {
            warn!("device_wait_idle() failed");
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

bitflags! {
    pub struct FeatureFlags: u32 {
        const DLSS = 1 << 0;
        const PRESENTATION = 1 << 1;
        const RAY_TRACING = 1 << 2;
    }
}

impl FeatureFlags {
    pub fn extension_names(&self) -> Vec<*const i8> {
        let mut device_extension_names_raw = vec![
            // This particular ext is part of 1.2 now, but add new ones here:
            // vk::KhrVulkanMemoryModelFn::name().as_ptr(),
        ];

        if self.contains(FeatureFlags::DLSS) {
            device_extension_names_raw.extend(
                [
                    b"VK_NVX_binary_import\0".as_ptr() as *const i8,
                    b"VK_KHR_push_descriptor\0".as_ptr() as *const i8,
                    vk::NvxImageViewHandleFn::name().as_ptr(),
                ]
                .iter(),
            );
        }

        if self.contains(FeatureFlags::PRESENTATION) {
            device_extension_names_raw.push(khr::Swapchain::name().as_ptr());
        }

        if self.contains(FeatureFlags::RAY_TRACING) {
            device_extension_names_raw.extend(
                [
                    vk::KhrPipelineLibraryFn::name().as_ptr(),        // rt dep
                    vk::KhrDeferredHostOperationsFn::name().as_ptr(), // rt dep
                    vk::KhrBufferDeviceAddressFn::name().as_ptr(),    // rt dep
                    vk::KhrAccelerationStructureFn::name().as_ptr(),
                    vk::KhrRayTracingPipelineFn::name().as_ptr(),
                    //vk::KhrRayQueryFn::name().as_ptr(),
                ]
                .iter(),
            );
        }

        device_extension_names_raw
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
