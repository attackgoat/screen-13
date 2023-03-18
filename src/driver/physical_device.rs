use {
    ash::vk,
    std::{
        ffi::CStr,
        fmt::{Debug, Formatter},
        ops::Deref,
    },
};

/// Execution queue selected by the current device.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct QueueFamily {
    /// Physical queue index.
    pub idx: u32,

    /// Properties of the selected execution queue.
    pub props: QueueFamilyProperties,
}

/// Describes additional propeties of the current execution queue.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct QueueFamilyProperties {
    /// Bitmask specifying capabilities of queues in a queue family.
    pub queue_flags: vk::QueueFlags,

    /// Number of queues which are available.
    pub queue_count: u32,

    /// The unsigned integer count of meaningful bits in the timestamps written via
    /// `vkCmdWriteTimestamp2` or `vkCmdWriteTimestamp`.
    ///
    /// The valid range for the count is 36 to 64 bits, or a value of 0, indicating no support for
    /// timestamps. Bits outside the valid range are guaranteed to be zeros.
    pub timestamp_valid_bits: u32,

    /// The minimum granularity supported for image transfer operations on the queues in this queue
    /// family.
    pub min_image_transfer_granularity: [u32; 3],
}

/// Structure which holds data about the physical hardware selected by the current device.
#[derive(Clone)]
pub struct PhysicalDevice {
    /// Memory properties of the physical device.
    pub mem_props: vk::PhysicalDeviceMemoryProperties,
    physical_device: vk::PhysicalDevice,

    /// Device properties of the physical device.
    pub props: vk::PhysicalDeviceProperties,
    queue_families: Vec<QueueFamily>,
}

impl PhysicalDevice {
    pub(super) fn new(
        physical_device: vk::PhysicalDevice,
        mem_props: vk::PhysicalDeviceMemoryProperties,
        props: vk::PhysicalDeviceProperties,
        queue_families: Vec<QueueFamily>,
    ) -> Self {
        Self {
            mem_props,
            physical_device,
            props,
            queue_families,
        }
    }

    // pub(super) fn has_presentation_support(
    //     _this: &Self,
    //     _instance: &Arc<Instance>,
    //     _surface: &Surface,
    // ) -> bool {
    // if let Ok(device) = Device::create(
    //     instance,
    //     this.clone(),
    //     DriverConfig::new().presentation(true).build().unwrap(),
    // ) {
    //     this.queue_families
    //         .iter()
    //         .enumerate()
    //         .any(|(queue_idx, info)| unsafe {
    //             info.props.queue_flags.contains(vk::QueueFlags::GRAPHICS)
    //                 && device
    //                     .surface_ext
    //                     .get_physical_device_surface_support(
    //                         this.physical_device,
    //                         queue_idx as _,
    //                         **surface,
    //                     )
    //                     .ok()
    //                     .unwrap_or_default()
    //         })
    // } else {
    //     false
    // }

    // TODO!
    // true
    // }

    pub(super) fn queue_families(this: &Self) -> impl Iterator<Item = QueueFamily> + '_ {
        this.queue_families.iter().copied()
    }

    pub(super) fn score_device_type(this: &Self) -> usize {
        match this.props.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 200,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
            _ => 0,
        }
    }
}

impl Debug for PhysicalDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe {
            write!(
                f,
                "{:?} ({:?})",
                CStr::from_ptr(self.props.device_name.as_ptr()),
                self.props.device_type
            )
        }
    }
}

impl Deref for PhysicalDevice {
    type Target = vk::PhysicalDevice;

    fn deref(&self) -> &Self::Target {
        &self.physical_device
    }
}
