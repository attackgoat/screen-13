use {
    super::Device,
    gfx_hal::{device::Device as _, pool::CommandPoolCreateFlags, queue::QueueFamilyId, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct CommandPool {
    device: Device,
    ptr: Option<<_Backend as Backend>::CommandPool>,
}

impl CommandPool {
    pub fn new(device: Device, family: QueueFamilyId) -> Self {
        Self::with_flags(device, family, CommandPoolCreateFlags::empty())
    }

    pub fn with_flags(
        device: Device,
        family: QueueFamilyId,
        flags: CommandPoolCreateFlags,
    ) -> Self {
        let cmd_pool = unsafe { device.create_command_pool(family, flags).unwrap() };

        Self {
            device,
            ptr: Some(cmd_pool),
        }
    }
}

impl AsMut<<_Backend as Backend>::CommandPool> for CommandPool {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::CommandPool {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::CommandPool> for CommandPool {
    fn as_ref(&self) -> &<_Backend as Backend>::CommandPool {
        &*self
    }
}

impl Deref for CommandPool {
    type Target = <_Backend as Backend>::CommandPool;

    fn deref(&self) -> &Self::Target {
        self.ptr.as_ref().unwrap()
    }
}

impl DerefMut for CommandPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr.as_mut().unwrap()
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        let ptr = self.ptr.take().unwrap();

        unsafe {
            self.device.destroy_command_pool(ptr);
        }
    }
}
