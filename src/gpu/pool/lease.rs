use {
    super::PoolRef,
    std::ops::{Deref, DerefMut},
};

/// A smart pointer type which automatically returns the associated resource to
/// the pool when dropped.
#[derive(Debug)]
pub struct Lease<T> {
    item: Option<T>,
    pool: PoolRef<T>,
}

impl<T> Lease<T> {
    pub(super) fn new(item: T, pool: &PoolRef<T>) -> Self {
        Self {
            item: Some(item),
            pool: PoolRef::clone(pool),
        }
    }
}

impl<T> Deref for Lease<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.item.as_ref().unwrap()
    }
}

impl<T> DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }
}

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        self.pool
            .as_ref()
            .borrow_mut()
            .push_front(self.item.take().unwrap());
    }
}
