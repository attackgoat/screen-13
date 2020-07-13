use {
    super::Driver,
    gfx_hal::{device::Device, pool::CommandPoolCreateFlags, queue::QueueFamilyId, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct CommandPool {
    driver: Driver,
    ptr: Option<<_Backend as Backend>::CommandPool>,
}

impl CommandPool {
    pub fn new(driver: Driver, family: QueueFamilyId) -> Self {
        Self::with_flags(driver, family, CommandPoolCreateFlags::empty())
    }

    pub fn with_flags(
        driver: Driver,
        family: QueueFamilyId,
        flags: CommandPoolCreateFlags,
    ) -> Self {
        let cmd_pool = {
            let device = driver.borrow();

            unsafe { device.create_command_pool(family, flags) }.unwrap()
        };

        Self {
            driver,
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
        let device = self.driver.borrow();
        let ptr = self.ptr.take().unwrap();

        unsafe {
            device.destroy_command_pool(ptr);
        }
    }
}
