use {
    super::Driver,
    gfx_hal::{device::Device, pool::CommandPoolCreateFlags, queue::QueueFamilyId, Backend},
    gfx_impl::Backend as _Backend,
    std::ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct CommandPool {
    driver: Driver,
    cmd_pool: Option<<_Backend as Backend>::CommandPool>,
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
        let cmd_pool = unsafe { driver.borrow().create_command_pool(family, flags).unwrap() };

        Self {
            driver,
            cmd_pool: Some(cmd_pool),
        }
    }
}

impl AsRef<<_Backend as Backend>::CommandPool> for CommandPool {
    fn as_ref(&self) -> &<_Backend as Backend>::CommandPool {
        self.cmd_pool.as_ref().unwrap()
    }
}

impl Deref for CommandPool {
    type Target = <_Backend as Backend>::CommandPool;

    fn deref(&self) -> &Self::Target {
        self.cmd_pool.as_ref().unwrap()
    }
}

impl DerefMut for CommandPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.cmd_pool.as_mut().unwrap()
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe {
            self.driver
                .borrow()
                .destroy_command_pool(self.cmd_pool.take().unwrap());
        }
    }
}
