pub mod hash;
pub mod lazy;

use {
    crate::driver::DriverError,
    log::warn,
    parking_lot::Mutex,
    std::{
        collections::VecDeque,
        fmt::Debug,
        ops::{Deref, DerefMut},
        sync::Arc,
        thread::panicking,
    },
};

type Cache<T> = Arc<Mutex<VecDeque<T>>>;

#[derive(Debug)]
pub struct Lease<T> {
    cache: Option<Cache<T>>,
    item: Option<T>,
}

impl<T> AsRef<T> for Lease<T> {
    fn as_ref(&self) -> &T {
        self.item.as_ref().unwrap()
    }
}

impl<T> AsMut<T> for Lease<T> {
    fn as_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }
}

impl<T> Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T> DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        if let Some(cache) = self.cache.as_ref() {
            cache.lock().push_back(self.item.take().unwrap());
        }
    }
}

pub trait Pool<I, T> {
    fn lease(&mut self, info: I) -> Result<Lease<T>, DriverError>;
}
