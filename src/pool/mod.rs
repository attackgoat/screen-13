pub mod hash;

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
        &*self
    }
}

impl<T> AsMut<T> for Lease<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut *self
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
            let mut cache = cache.lock();

            // TODO: I'm sure some better logic would be handy
            if cache.len() < 8 {
                cache.push_back(self.item.take().unwrap());
            } else {
                // TODO: Better design for this - we are dropping these extra resources to avoid
                // bigger issues - but this is just a symptom really - hasn't been a priority yet
                warn!("hash pool build-up");
            }
        }
    }
}

pub trait Pool<I, T> {
    fn lease(&mut self, info: I) -> Result<Lease<T>, DriverError>;
}
