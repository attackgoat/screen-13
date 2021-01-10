use {
    crate::gpu::device,
    gfx_hal::{device::Device as _, pool::CommandPoolCreateFlags, queue::QueueFamilyId, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

pub struct CommandPool(Option<<_Backend as Backend>::CommandPool>);

impl CommandPool {
    pub unsafe fn new(family: QueueFamilyId) -> Self {
        Self::new_flags(family, CommandPoolCreateFlags::empty())
    }

    pub unsafe fn new_flags(family: QueueFamilyId, flags: CommandPoolCreateFlags) -> Self {
        let cmd_pool = device().create_command_pool(family, flags).unwrap();

        Self(Some(cmd_pool))
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
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for CommandPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        let ptr = self.0.take().unwrap();

        unsafe {
            device().destroy_command_pool(ptr);
        }
    }
}
