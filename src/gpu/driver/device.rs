use {
    crate::error::Error,
    gfx_hal::{
        adapter::{MemoryProperties, PhysicalDevice as _},
        memory::Properties,
        Backend, MemoryTypeId,
    },
    gfx_impl::Backend as _Backend,
    std::ops::Deref,
};

#[derive(Clone, Copy)]
pub struct Device {
    logical_device: &'static <_Backend as Backend>::Device,
    mem_props: &'static MemoryProperties,
    physical_device: &'static <_Backend as Backend>::PhysicalDevice,
}

impl Device {
    pub fn new(
        physical_device: &'static <_Backend as Backend>::PhysicalDevice,
        logical_device: &'static <_Backend as Backend>::Device,
        mem_props: &'static MemoryProperties,
    ) -> Self {
        Self {
            logical_device,
            mem_props,
            physical_device,
        }
    }

    // pub fn gpu(device: &Self) -> &<_Backend as Backend>::PhysicalDevice {
    //     &device.physical_device
    // }

    pub fn mem_ty(device: &Self, mask: u32, props: Properties) -> Option<MemoryTypeId> {
        //debug!("type_mask={} properties={:?}", type_mask, properties);
        device
            .mem_props
            .memory_types
            .iter()
            .enumerate()
            .position(|(idx, mem_ty)| {
                //debug!("Mem ID {} type={:?}", id, mem_type);
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                mask & (1 << idx) != 0 && mem_ty.properties.contains(props)
            })
            .map(MemoryTypeId)
    }
}

impl AsRef<<_Backend as Backend>::Device> for Device {
    fn as_ref(&self) -> &<_Backend as Backend>::Device {
        &*self
    }
}

impl Deref for Device {
    type Target = <_Backend as Backend>::Device;

    fn deref(&self) -> &Self::Target {
        &self.logical_device
    }
}
