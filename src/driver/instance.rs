use {
    super::{DriverError, PhysicalDevice, QueueFamily, QueueFamilyProperties},
    ash::{extensions::ext, vk, Entry},
    log::{debug, error, info, trace, warn},
    std::{
        ffi::{c_void, CStr, CString},
        fmt::{Debug, Formatter},
        ops::Deref,
        os::raw::c_char,
        thread::{panicking, sleep},
        time::Duration,
    },
};

unsafe extern "system" fn vulkan_debug_callback(
    _flags: vk::DebugReportFlagsEXT,
    _obj_type: vk::DebugReportObjectTypeEXT,
    _src_obj: u64,
    _location: usize,
    _msg_code: i32,
    _layer_prefix: *const c_char,
    message: *const c_char,
    _user_data: *mut c_void,
) -> u32 {
    let message = CStr::from_ptr(message).to_str().unwrap();

    if message.starts_with("Validation Warning: [ UNASSIGNED-BestPractices-pipeline-stage-flags ]")
    {
        // vk_sync uses vk::PipelineStageFlags::ALL_COMMANDS with AccessType::NOTHING and others
        warn!("{}\n", message);
    } else {
        error!("{}\n", message);
        panic!("{}\n", message);
    }

    vk::FALSE
}

pub struct Instance {
    _debug_callback: Option<vk::DebugReportCallbackEXT>,
    #[allow(deprecated)] // TODO: Remove? Look into this....
    _debug_loader: Option<ext::DebugReport>,
    _debug_utils: Option<ext::DebugUtils>,
    pub entry: Entry,
    instance: ash::Instance,
}

impl Instance {
    pub fn new<'a>(
        debug: bool,
        required_extensions: impl Iterator<Item = &'a CStr>,
    ) -> Result<Self, DriverError> {
        #[cfg(not(target_os = "macos"))]
        let vulkan_api_version = vk::API_VERSION_1_2;

        #[cfg(target_os = "macos")]
        // See https://github.com/KhronosGroup/MoltenVK/issues/1567
        let vulkan_api_version = vk::API_VERSION_1_1;

        let entry = unsafe {
            #[cfg(not(target_os = "macos"))]
            let entry = Entry::load().map_err(|_| {
                error!("Vulkan driver not found");

                DriverError::Unsupported
            })?;

            #[cfg(target_os = "macos")]
            let entry = ash_molten::load();

            entry
        };
        let required_extensions = required_extensions.collect::<Vec<_>>();
        let instance_extensions = required_extensions
            .iter()
            .map(|ext| ext.as_ptr())
            .chain(unsafe { Self::extension_names(debug).into_iter() })
            .collect::<Box<[_]>>();
        let layer_names = Self::layer_names(debug);
        let layer_names: Vec<*const i8> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();
        let app_desc = vk::ApplicationInfo::builder().api_version(vulkan_api_version);
        let instance_desc = vk::InstanceCreateInfo::builder()
            .application_info(&app_desc)
            .enabled_layer_names(&layer_names)
            .enabled_extension_names(&instance_extensions);

        let instance = unsafe {
            entry.create_instance(&instance_desc, None).map_err(|_| {
                if debug {
                    warn!("Debug may only be enabled with a valid Vulkan SDK installation");
                }

                error!("Vulkan driver does not support API v1.2");

                for layer_name in Self::layer_names(debug) {
                    debug!("Layer: {:?}", layer_name);
                }

                for extension_name in required_extensions {
                    debug!("Extension: {:?}", extension_name);
                }

                DriverError::Unsupported
            })?
        };

        trace!("Created a Vulkan instance");

        let (debug_loader, debug_callback, debug_utils) = if debug {
            let debug_info = vk::DebugReportCallbackCreateInfoEXT {
                flags: vk::DebugReportFlagsEXT::ERROR
                    | vk::DebugReportFlagsEXT::WARNING
                    | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING,
                pfn_callback: Some(vulkan_debug_callback),
                ..Default::default()
            };

            #[allow(deprecated)]
            let debug_loader = ext::DebugReport::new(&entry, &instance);

            let debug_callback = unsafe {
                #[allow(deprecated)]
                debug_loader
                    .create_debug_report_callback(&debug_info, None)
                    .unwrap()
            };

            let debug_utils = ext::DebugUtils::new(&entry, &instance);

            (Some(debug_loader), Some(debug_callback), Some(debug_utils))
        } else {
            (None, None, None)
        };

        Ok(Self {
            _debug_callback: debug_callback,
            _debug_loader: debug_loader,
            _debug_utils: debug_utils,
            entry,
            instance,
        })
    }

    unsafe fn extension_names(debug: bool) -> Vec<*const i8> {
        let mut res = vec![];

        if debug {
            #[allow(deprecated)]
            res.push(ext::DebugReport::name().as_ptr());
            res.push(vk::ExtDebugUtilsFn::name().as_ptr());
        }

        res
    }

    fn layer_names(debug: bool) -> Vec<CString> {
        let mut res = Vec::new();

        if debug {
            if let Ok(name) = CString::new("VK_LAYER_KHRONOS_validation") {
                res.push(name);
            }
        }

        res
    }

    pub fn physical_devices(
        this: &Self,
    ) -> Result<impl Iterator<Item = PhysicalDevice> + '_, DriverError> {
        unsafe {
            Ok(this
                .enumerate_physical_devices()
                .map_err(|_| DriverError::Unsupported)?
                .into_iter()
                .map(|physical_device| {
                    let props = this.get_physical_device_properties(physical_device);
                    let queue_families = this
                        .get_physical_device_queue_family_properties(physical_device)
                        .into_iter()
                        .enumerate()
                        .map(|(idx, props)| QueueFamily {
                            idx: idx as _,
                            props: QueueFamilyProperties {
                                queue_flags: props.queue_flags,
                                queue_count: props.queue_count,
                                timestamp_valid_bits: props.timestamp_valid_bits,
                                min_image_transfer_granularity: [
                                    props.min_image_transfer_granularity.width,
                                    props.min_image_transfer_granularity.height,
                                    props.min_image_transfer_granularity.depth,
                                ],
                            },
                        })
                        .collect();
                    let mem_props = this.get_physical_device_memory_properties(physical_device);

                    PhysicalDevice::new(physical_device, mem_props, props, queue_families)
                })
                .filter(|physical_device| {
                    let major = vk::api_version_major(physical_device.props.api_version);
                    let minor = vk::api_version_minor(physical_device.props.api_version);

                    major == 1 && minor >= 1 || major > 1
                }))
        }
    }
}

impl Debug for Instance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Device")
    }
}

impl Deref for Instance {
    type Target = ash::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            #[allow(deprecated)]
            if let Some(debug_loader) = &self._debug_loader {
                let debug_callback = self._debug_callback.unwrap();
                debug_loader.destroy_debug_report_callback(debug_callback, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}
