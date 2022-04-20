use {
    crate::{
        driver::{
            Buffer, BufferInfo, BufferInfoBuilder, CommandBuffer, DescriptorPool,
            DescriptorPoolInfo, DescriptorPoolInfoBuilder, Device, DriverError, Image, ImageInfo,
            ImageInfoBuilder, QueueFamily, RenderPass, RenderPassInfo, RenderPassInfoBuilder,
        },
        graph::{BufferBinding, ImageBinding},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    parking_lot::Mutex,
    std::{
        collections::{HashMap, VecDeque},
        fmt::Debug,
        ops::{Deref, DerefMut},
        thread::panicking,
    },
};

type Cache<T, P> = Shared<Mutex<VecDeque<T>>, P>;

pub trait Contract<P> {
    type Term;
}

#[derive(Debug)]
pub struct HashPool<P>
where
    P: SharedPointerKind,
{
    buffer_binding_cache: HashMap<BufferInfo, Cache<BufferBinding<P>, P>>,
    command_buffer_cache: HashMap<QueueFamily, Cache<CommandBuffer<P>, P>>,
    descriptor_pool_cache: HashMap<DescriptorPoolInfo, Cache<Shared<DescriptorPool<P>, P>, P>>,
    pub device: Shared<Device<P>, P>,
    image_binding_cache: HashMap<ImageInfo, Cache<ImageBinding<P>, P>>,
    render_pass_cache: HashMap<RenderPassInfo, Cache<RenderPass<P>, P>>,
}

// TODO: Add some sort of manager features (like, I dunno, "Clear Some Memory For me")
impl<P> HashPool<P>
where
    P: SharedPointerKind,
{
    pub fn new(device: &Shared<Device<P>, P>) -> Self {
        let device = Shared::clone(device);

        Self {
            buffer_binding_cache: Default::default(),
            command_buffer_cache: Default::default(),
            descriptor_pool_cache: Default::default(),
            device,
            image_binding_cache: Default::default(),
            render_pass_cache: Default::default(),
        }
    }

    pub fn lease<C>(&mut self, info: C) -> Result<Lease<<C as Contract<P>>::Term, P>, DriverError>
    where
        C: Pooled<Lease<<C as Contract<P>>::Term, P>, P>,
        C: Contract<P> + Debug,
    {
        info.lease(self)
    }
}

#[derive(Debug)]
pub struct Lease<T, P>
where
    P: SharedPointerKind,
{
    cache: Option<Cache<T, P>>,
    item: Option<T>,
}

impl<T, P> Lease<T, P>
where
    P: SharedPointerKind,
{
    /// Moves the cache reference into a new lease. The old lease will retain the item reference but
    /// will no longer have any return-to-pool-when-dropped behavior.
    pub(super) fn transfer(&mut self, item: T) -> Self {
        Self {
            cache: self.cache.take(),
            item: Some(item),
        }
    }
}

impl<T, P> AsRef<T> for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T, P> AsMut<T> for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn as_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T, P> Deref for Lease<T, P>
where
    P: SharedPointerKind,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T, P> DerefMut for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T, P> Drop for Lease<T, P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        if let Some(cache) = self.cache.as_ref() {
            let mut cache = cache.lock();

            // TODO: I'm sure some better logic would be handy
            if cache.len() < 8 {
                cache.push_back(self.item.take().unwrap());
            }
        }
    }
}

pub trait Pooled<T, P> {
    fn lease(self, pool: &mut HashPool<P>) -> Result<T, DriverError>
    where
        P: SharedPointerKind;
}

// Enable the basic leasing of items
macro_rules! lease {
    ($src:ident -> $dst:ident) => {
        impl<P> Contract<P> for $src
        where
            P: SharedPointerKind,
        {
            type Term = $dst<P>;
        }

        paste::paste! {
            impl<P> Pooled<Lease<$dst<P>, P>, P> for $src
            where
                P: SharedPointerKind,
            {
                fn lease(self, pool: &mut HashPool<P>) -> Result<Lease<$dst<P>, P>, DriverError> {
                    let cache = pool.[<$dst:snake _cache>].entry(self.clone())
                        .or_insert_with(|| {
                            Shared::new(Mutex::new(VecDeque::new()))
                        });
                    let cache_ref = Shared::clone(cache);
                    let mut cache = cache.lock();

                    if cache.is_empty() || ![<can_lease_ $dst:snake>](cache.front_mut().unwrap()) {
                        // Calls the function defined in the other macros
                        let item = [<create_ $dst:snake>](&pool.device, self)?;

                        return Ok(Lease {
                            cache: Some(cache_ref),
                            item: Some(item),
                        });
                    }

                    Ok(Lease {
                        cache: Some(cache_ref),
                        item: cache.pop_front(),
                    })
                }
            }
        }
    };
}

// Enable leasing items using their basic info as the entire request
macro_rules! lease_info {
    ($src:ident -> $dst:ident) => {
        lease!($src -> $dst);

        paste::paste! {
            // Called by the lease macro
            fn [<create_ $dst:snake>]<P>(device: &Shared<Device<P>, P>, info: $src)
                -> Result<$dst<P>, DriverError>
            where
                P: SharedPointerKind
            {
                $dst::create(device, info)
            }
        }
    };
}

lease_info!(QueueFamily -> CommandBuffer);

// Used by macro invocation, above
fn can_lease_command_buffer(cmd_buf: &mut CommandBuffer<impl SharedPointerKind>) -> bool {
    let can_lease = unsafe {
        // Don't lease this command buffer if it is unsignalled; we'll create a new one
        // and wait for this, and those behind it, to signal.
        cmd_buf
            .device
            .get_fence_status(cmd_buf.fence)
            .unwrap_or_default()
    };

    if can_lease {
        // Drop anything we were holding from the last submission
        CommandBuffer::drop_fenced(cmd_buf);
    }

    can_lease
}

// Enable leasing items as above, but also using their info builder type for convenience
macro_rules! lease_info_builder {
    ($src:ident -> $dst:ident) => {
        lease_info!($src -> $dst);

        paste::paste! {
            // Called by the lease macro, via the lease_info macro
            const fn [<can_lease_ $dst:snake>]<T>(_: &T) -> bool {
                true
            }

            impl<P> Contract<P> for [<$src Builder>]
            where
                P: SharedPointerKind,
            {
                type Term = $dst<P>;
            }

            impl<P> Pooled<Lease<$dst<P>, P>, P> for [<$src Builder>] where P: SharedPointerKind {
                fn lease(self, pool: &mut HashPool<P>) -> Result<Lease<$dst<P>, P>, DriverError> {
                    let info = self.build();

                    // We will unwrap the info builder - it may panic!
                    assert!(info.is_ok(), "Invalid pool resource info: {:#?}", info);

                    info.unwrap().lease(pool)
                }
            }
        }
    };
}

lease_info_builder!(RenderPassInfo -> RenderPass);

macro_rules! lease_info_binding {
    ($src:ident -> $dst:ident) => {
        paste::paste! {
            lease!($src -> [<$dst Binding>]);

            // Called by the lease macro
            fn [<create_ $dst:snake _binding>]<P>(device: &Shared<Device<P>, P>, info: $src)
                -> Result<[<$dst Binding>]<P>, DriverError>
            where
                P: SharedPointerKind
            {
                Ok([<$dst Binding>]::new($dst::create(device, info)?))
            }

            // Called by the lease macro
            fn [<can_lease_ $dst:snake _binding>]<T>(_: &mut T) -> bool {
                true
            }

            impl<P> Contract<P> for [<$src Builder>]
            where
                P: SharedPointerKind,
            {
                type Term = [<$dst Binding>]<P>;
            }

            impl<P> Pooled<Lease<[<$dst Binding>]<P>, P>, P> for [<$src Builder>] where P: SharedPointerKind {
                fn lease(self, pool: &mut HashPool<P>) -> Result<Lease<[<$dst Binding>]<P>, P>, DriverError> {
                    self.build().lease(pool)
                }
            }
        }
    };
}

lease_info_binding!(BufferInfo -> Buffer);
lease_info_binding!(ImageInfo -> Image);

// Enable types of leases where the item is a Shared item (these can be dangerous!!)
macro_rules! shared_lease {
    ($src:ident -> $dst:ident) => {
        impl<P> Contract<P> for $src
        where
            P: SharedPointerKind,
        {
            type Term = Shared<$dst<P>, P>;
        }

        paste::paste! {
            impl<P> Pooled<Lease<Shared<$dst<P>, P>, P>, P> for $src
            where
                P: SharedPointerKind,
            {
                fn lease(self, pool: &mut HashPool<P>) -> Result<Lease<Shared<$dst<P>, P>, P>, DriverError> {
                    let cache = pool.[<$dst:snake _cache>].entry(self.clone())
                        .or_insert_with(|| {
                            Shared::new(Mutex::new(VecDeque::new()))
                        });
                    let cache_ref = Shared::clone(cache);
                    let mut cache = cache.lock();

                    Ok(if let item @ Some(_) = cache.pop_front() {
                        Lease {
                            cache: Some(cache_ref),
                            item,
                        }
                    } else {
                        Lease {
                            cache: Some(cache_ref),
                            item: Some(Shared::new($dst::create(&pool.device, self)?)),
                        }
                    })
                }
            }

            impl<P> Contract<P> for [<$src Builder>]
            where
                P: SharedPointerKind,
            {
                type Term = Shared<$dst<P>, P>;
            }

            impl<P> Pooled<Lease<Shared<$dst<P>, P>, P>, P> for [<$src Builder>] where P: SharedPointerKind {
                fn lease(self, pool: &mut HashPool<P>) -> Result<Lease<Shared<$dst<P>, P>, P>, DriverError> {
                    let desc = self.build();

                    // We will unwrap the description builder - it may panic!
                    assert!(desc.is_ok(), "Invalid pool resource description: {:#?}", desc);

                    desc.unwrap().lease(pool)
                }
            }
        }
    }
}

// These items need to be leased and then shared around - don't drop the lease while there are still
// shares floating around out there or their new lease owner may do something like reset the pool.
shared_lease!(DescriptorPoolInfo -> DescriptorPool);
