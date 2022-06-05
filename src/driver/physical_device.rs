use {
    super::{Instance, Surface},
    ash::vk,
    std::{
        ffi::CStr,
        fmt::{Debug, Formatter},
        ops::Deref,
        sync::Arc,
    },
};

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct QueueFamily {
    pub idx: u32,
    pub props: QueueFamilyProperties,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct QueueFamilyProperties {
    pub queue_flags: vk::QueueFlags,
    pub queue_count: u32,
    pub timestamp_valid_bits: u32,
    pub min_image_transfer_granularity: [u32; 3],
}

#[derive(Clone)]
pub struct PhysicalDevice {
    pub mem_props: vk::PhysicalDeviceMemoryProperties,
    physical_device: vk::PhysicalDevice,
    pub props: vk::PhysicalDeviceProperties,
    queue_families: Vec<QueueFamily>,
}

impl PhysicalDevice {
    pub fn new(
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

    pub fn has_presentation_support(
        _this: &Self,
        _instance: &Arc<Instance>,
        _surface: &Surface,
    ) -> bool {
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
        true
    }

    pub fn has_ray_tracing_support(_this: &Self) -> bool {
        // TODO!
        true
    }

    pub fn queue_families(this: &Self) -> impl Iterator<Item = QueueFamily> + '_ {
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
