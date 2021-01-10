use {
    super::PoolRef,
    archery::SharedPointerKind,
    std::ops::{Deref, DerefMut},
};

/// A smart pointer type which automatically returns the associated resource to
/// the pool when dropped.
pub struct Lease<T, P>
where
    P: SharedPointerKind,
{
    item: Option<T>,
    pool: PoolRef<T, P>,
}

impl<T, P> Lease<T, P>
where
    P: SharedPointerKind,
{
    pub fn new(item: T, pool: &PoolRef<T, P>) -> Self {
        Self {
            item: Some(item),
            pool: PoolRef::clone(&pool),
        }
    }
}

impl<T, P> Deref for Lease<T, P>
where
    P: SharedPointerKind,
{
    type Target = T;

    fn deref(&self) -> &T {
        self.item.as_ref().unwrap()
    }
}

impl<T, P> DerefMut for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn deref_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }
}

impl<T, P> Drop for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        // Return item to the pool
        let item = self.item.take().unwrap();
        self.pool.borrow_mut().push_front(item);
    }
}
