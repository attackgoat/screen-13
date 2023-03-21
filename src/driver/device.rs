//! Logical device resource types

use {
    super::{physical_device::PhysicalDevice, DriverError, Instance, SamplerDesc},
    ash::{extensions::khr, vk},
    ash_window::enumerate_required_extensions,
    derive_builder::{Builder, UninitializedFieldError},
    gpu_allocator::{
        vulkan::{Allocator, AllocatorCreateDesc},
        AllocatorDebugSettings,
    },
    log::{error, trace, warn},
    parking_lot::Mutex,
    raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle},
    std::{
        cmp::Ordering,
        collections::HashMap,
        ffi::CStr,
        fmt::{Debug, Formatter},
        iter::{empty, repeat},
        mem::forget,
        ops::Deref,
        thread::panicking,
        time::Instant,
    },
};

/// Function type for selection of physical devices.
pub type SelectPhysicalDeviceFn = dyn FnOnce(&[PhysicalDevice]) -> usize;

/// Opaque handle to a device object.
pub struct Device {
    pub(crate) accel_struct_ext: Option<khr::AccelerationStructure>,

    pub(super) allocator: Option<Mutex<Allocator>>,

    device: ash::Device,
    immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,

    /// Vulkan instance pointer, which includes useful functions.
    pub(super) instance: Instance,

    /// The physical device, which contains useful data about features, properties, and limits.
    pub physical_device: PhysicalDevice,

    /// The physical execution queues which all work will be submitted to.
    pub(crate) queues: Box<[Queue]>,

    pub(crate) ray_trace_ext: Option<khr::RayTracingPipeline>,

    pub(super) surface_ext: Option<khr::Surface>,
    pub(super) swapchain_ext: Option<khr::Swapchain>,
}

impl Device {
    fn create(
        instance: Instance,
        select_physical_device: Box<SelectPhysicalDeviceFn>,
        debug: bool,
        display_window: bool,
    ) -> Result<Self, DriverError> {
        let mut physical_devices = Instance::physical_devices(&instance)?;

        if physical_devices.is_empty() {
            error!("no supported devices found");

            return Err(DriverError::Unsupported);
        }

        let mut phyical_device_idx = select_physical_device(&physical_devices);

        if phyical_device_idx >= physical_devices.len() {
            warn!("invalid device selected");

            phyical_device_idx = 0;
        }

        let physical_device = physical_devices.remove(phyical_device_idx);

        let mut enabled_ext_names = Vec::with_capacity(5);

        if display_window {
            enabled_ext_names.push(vk::KhrSwapchainFn::name().as_ptr());
        }

        if physical_device.accel_struct_properties.is_some() {
            enabled_ext_names.push(vk::KhrAccelerationStructureFn::name().as_ptr());
            enabled_ext_names.push(vk::KhrDeferredHostOperationsFn::name().as_ptr());
        }

        if physical_device.ray_query_features.is_some() {
            enabled_ext_names.push(vk::KhrRayQueryFn::name().as_ptr());
        }

        if physical_device.ray_trace_features.is_some() {
            enabled_ext_names.push(vk::KhrRayTracingPipelineFn::name().as_ptr());
        }

        let priorities = repeat(1.0)
            .take(
                physical_device
                    .queue_families
                    .iter()
                    .map(|family| family.queue_count)
                    .max()
                    .unwrap_or_default() as _,
            )
            .collect::<Box<_>>();

        if priorities.is_empty() {
            error!("device contains no queues");

            return Err(DriverError::Unsupported);
        }

        let queue_infos = physical_device
            .queue_families
            .iter()
            .enumerate()
            .map(|(idx, family)| {
                let mut queue_info = vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(idx as _)
                    .queue_priorities(&priorities[0..family.queue_count as usize])
                    .build();
                queue_info.queue_count = family.queue_count;

                queue_info
            })
            .collect::<Box<_>>();

        let vk::InstanceFnV1_1 {
            get_physical_device_features2,
            ..
        } = instance.fp_v1_1();
        let mut features_v1_1 = vk::PhysicalDeviceVulkan11Features::default();
        let mut features_v1_2 = vk::PhysicalDeviceVulkan12Features::default();
        let mut acceleration_structure_features =
            vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut ray_query_features = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
        let mut ray_trace_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
        let mut features = vk::PhysicalDeviceFeatures2::builder()
            .push_next(&mut features_v1_1)
            .push_next(&mut features_v1_2)
            .push_next(&mut acceleration_structure_features)
            .push_next(&mut ray_query_features)
            .push_next(&mut ray_trace_features)
            .build();
        unsafe { get_physical_device_features2(*physical_device, &mut features) };

        unsafe {
            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_infos)
                .enabled_extension_names(&enabled_ext_names)
                .push_next(&mut features);
            let device = instance
                .create_device(*physical_device, &device_create_info, None)
                .map_err(|err| {
                    error!("unable to create device: {err}");

                    DriverError::Unsupported
                })?;
            let allocator = Allocator::new(&AllocatorCreateDesc {
                instance: (*instance).clone(),
                device: device.clone(),
                physical_device: *physical_device,
                debug_settings: AllocatorDebugSettings {
                    log_leaks_on_shutdown: debug,
                    log_memory_information: debug,
                    log_allocations: debug,
                    ..Default::default()
                },
                buffer_device_address: true,
            })
            .map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;

            let mut queues = Vec::with_capacity(
                physical_device
                    .queue_families
                    .iter()
                    .map(|family| family.queue_count)
                    .sum::<u32>() as _,
            );

            for (family_index, family) in physical_device.queue_families.iter().enumerate() {
                for queue_index in 0..family.queue_count {
                    queues.push(Queue {
                        family_index,
                        queue: device.get_device_queue(family_index as _, queue_index),
                    });
                }
            }

            let queues = queues.into_iter().collect();

            let immutable_samplers = Self::create_immutable_samplers(&device)?;

            let surface_ext = display_window.then(|| khr::Surface::new(&instance.entry, &instance));
            let swapchain_ext = display_window.then(|| khr::Swapchain::new(&instance, &device));
            let accel_struct_ext = physical_device
                .accel_struct_properties
                .is_some()
                .then(|| khr::AccelerationStructure::new(&instance, &device));
            let ray_trace_ext = physical_device
                .ray_trace_features
                .is_some()
                .then(|| khr::RayTracingPipeline::new(&instance, &device));

            Ok(Self {
                accel_struct_ext,
                allocator: Some(Mutex::new(allocator)),
                device,
                immutable_samplers,
                instance,
                physical_device,
                queues,
                ray_trace_ext,
                surface_ext,
                swapchain_ext,
            })
        }
    }

    /// Constructs a new device using the given configuration.
    pub fn create_headless(info: impl Into<DeviceInfo>) -> Result<Self, DriverError> {
        let DeviceInfo {
            debug,
            select_physical_device,
        } = info.into();
        let instance = Instance::new(debug, empty())?;

        Self::create(instance, select_physical_device, debug, false)
    }

    /// Constructs a new device using the given configuration.
    pub fn create_display_window(
        info: impl Into<DeviceInfo>,
        display_window: &(impl HasRawDisplayHandle + HasRawWindowHandle),
    ) -> Result<Self, DriverError> {
        let DeviceInfo {
            debug,
            select_physical_device,
        } = info.into();
        let required_extensions =
            enumerate_required_extensions(display_window.raw_display_handle())
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?
                .iter()
                .map(|ext| unsafe { CStr::from_ptr(*ext as *const _) });
        let instance = Instance::new(debug, required_extensions)?;

        Self::create(instance, select_physical_device, debug, true)
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
    pub fn format_properties(this: &Self, format: vk::Format) -> vk::FormatProperties {
        unsafe {
            this.instance
                .get_physical_device_format_properties(*this.physical_device, format)
        }
    }

    /// Lists the physical device's image format capabilities.
    pub fn image_format_properties(
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

/// Information used to create a [`Device`] instance.
#[derive(Builder)]
#[builder(
    build_fn(private, name = "fallible_build", error = "DeviceInfoBuilderError"),
    pattern = "owned"
)]
pub struct DeviceInfo {
    /// Enables Vulkan validation layers.
    ///
    /// This requires a Vulkan SDK installation and will cause validation errors to introduce
    /// panics as they happen.
    ///
    /// _NOTE:_ Consider turning OFF debug if you discover an unknown issue. Often the validation
    /// layers will throw an error before other layers can provide additional context such as the
    /// API dump info or other messages. You might find the "actual" issue is detailed in those
    /// subsequent details.
    ///
    /// ## Platform-specific
    ///
    /// **macOS:** Has no effect.
    #[builder(default)]
    pub debug: bool,

    /// Callback function used to select a [`PhysicalDevice`] from the available devices. The
    /// callback must return the index of the selected device.
    #[builder(default = "Box::new(DeviceInfo::discrete_gpu)")]
    pub select_physical_device: Box<SelectPhysicalDeviceFn>,
}

impl DeviceInfo {
    /// Specifies default device information.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> DeviceInfoBuilder {
        Default::default()
    }

    /// A builtin [`DeviceInfo::select_physical_device`] function which prioritizes selection of
    /// lower-power integrated GPU devices.
    pub fn integrated_gpu(physical_devices: &[PhysicalDevice]) -> usize {
        assert!(!physical_devices.is_empty());

        let mut physical_devices = physical_devices.iter().enumerate().collect::<Box<_>>();

        if physical_devices.len() == 1 {
            return 0;
        }

        fn device_type(ty: vk::PhysicalDeviceType) -> usize {
            match ty {
                vk::PhysicalDeviceType::INTEGRATED_GPU => 0,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                vk::PhysicalDeviceType::CPU => 2,
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                _ => 4,
            }
        }

        physical_devices.sort_unstable_by(|(_, lhs), (_, rhs)| {
            let lhs_device_ty = device_type(lhs.properties_v1_0.device_type);
            let rhs_device_ty = device_type(rhs.properties_v1_0.device_type);
            let device_ty = lhs_device_ty.cmp(&rhs_device_ty);

            if device_ty != Ordering::Equal {
                return device_ty;
            }

            // TODO: Select the device with the most memory

            Ordering::Equal
        });

        let (idx, _) = physical_devices[0];

        idx
    }

    /// A builtin [`DeviceInfo::select_physical_device`] function which prioritizes selection of
    /// higher-performance discrete GPU devices.
    pub fn discrete_gpu(physical_devices: &[PhysicalDevice]) -> usize {
        assert!(!physical_devices.is_empty());

        let mut physical_devices = physical_devices.iter().enumerate().collect::<Box<_>>();

        if physical_devices.len() == 1 {
            return 0;
        }

        fn device_type(ty: vk::PhysicalDeviceType) -> usize {
            match ty {
                vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                vk::PhysicalDeviceType::CPU => 3,
                _ => 4,
            }
        }

        physical_devices.sort_unstable_by(|(_, lhs), (_, rhs)| {
            let lhs_device_ty = device_type(lhs.properties_v1_0.device_type);
            let rhs_device_ty = device_type(rhs.properties_v1_0.device_type);
            let device_ty = lhs_device_ty.cmp(&rhs_device_ty);

            if device_ty != Ordering::Equal {
                return device_ty;
            }

            // TODO: Select the device with the most memory

            Ordering::Equal
        });

        let (idx, _) = physical_devices[0];

        idx
    }
}

impl From<DeviceInfoBuilder> for DeviceInfo {
    fn from(info: DeviceInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl DeviceInfoBuilder {
    /// Builds a new `DeviceInfo`.
    pub fn build(self) -> DeviceInfo {
        self.fallible_build().unwrap()
    }
}

#[derive(Debug)]
struct DeviceInfoBuilderError;

impl From<UninitializedFieldError> for DeviceInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// An execution queue.
pub struct Queue {
    /// The index into `PhysicalDevice::queue_families` describing the properties of the family of
    /// this queue.
    pub family_index: usize,

    queue: vk::Queue,
}

impl Deref for Queue {
    type Target = vk::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}
