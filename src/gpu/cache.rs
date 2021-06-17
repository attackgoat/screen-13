use {
    super::{
        data::{CopyRange, Data},
        pool::{Lease, Pool},
    },
    archery::SharedPointerKind,
    gfx_hal::buffer::Usage as BufferUsage,
    std::{mem::replace, ops::Range},
};

// Always ask for a bigger cache capacity than needed; it reduces the need to completely replace
// the existing cache and then have to copy all the old data over.
const CACHE_CAPACITY_FACTOR: f32 = 4.0;

/// Used to keep track of data allocated during compilation and also the previous value which we
/// will copy over during the drawing operation.
pub struct Allocation<T> {
    pub current: T,
    pub previous: Option<(T, u64)>,
    usage: BufferUsage,
}

/// Extends the `Data` type so we can track which portions require updates.
///
/// **_NOTE:_** The fields of this type are extremely public. It would be nice to hide some of this
/// complexity within functions but I haven't figured out how to do that just yet.
pub struct LruCache<Key, P>
where
    P: SharedPointerKind,
{
    pub allocation: Allocation<Lease<Data, P>>,

    pub items: Vec<Lru<Key>>,

    /// Segments of gpu memory which must be "compacted" (read: copied) within the gpu.
    pub pending_copies: Vec<CopyRange>,

    /// This range, if present, is the portion that needs to be written from cpu to gpu.
    pub pending_write: Option<Range<u64>>,

    /// Memory usage on the gpu, sorted by the first field which is the offset.
    pub usage: Vec<(u64, Key)>,
}

impl<Key, P> LruCache<Key, P>
where
    P: SharedPointerKind,
{
    pub unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        len: u64,
        usage: BufferUsage,
    ) -> Self {
        let data = pool.data_usage(
            #[cfg(feature = "debug-names")]
            &name,
            len,
            usage,
            false,
        );

        Self {
            allocation: Allocation {
                current: data,
                previous: None,
                usage,
            },
            items: Vec::with_capacity(1024),
            pending_copies: Vec::with_capacity(1024),
            pending_write: None,
            usage: Vec::with_capacity(1024),
        }
    }

    /// Allocates or re-allocates leased data of the given size.
    pub unsafe fn realloc(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        len: u64,
    ) where
        Key: Stride,
    {
        #[cfg(feature = "debug-names")]
        if let Some(allocation) = self.allocation.as_mut() {
            allocation.current.set_name(&name);
        }

        // Early-out if we do not need to resize the buffer
        if self.allocation.current.capacity() >= len {
            return;
        }

        // We over-allocate the requested capacity to prevent rapid reallocations
        let capacity = (len as f32 * CACHE_CAPACITY_FACTOR) as u64;
        let data = pool.data_usage(
            #[cfg(feature = "debug-names")]
            &name,
            capacity,
            self.allocation.usage,
            false,
        );

        // Preserve the old data so that we can copy it directly over before drawing
        let previous = replace(&mut self.allocation.current, data);
        if !self.usage.is_empty() {
            let previous_len = self
                .usage
                .last()
                .map(|(offset, key)| offset + key.stride())
                .unwrap();
            self.allocation.previous = Some((previous, previous_len));
        }
    }
}

impl<Key, P> LruCache<Key, P>
where
    Key: Ord + Stride,
    P: SharedPointerKind,
{
    /// Moves cache items into clumps so future items can be appended onto the end without needing
    /// to resize the cache buffer. As a side effect this causes dirty regions to be moved on the
    /// GPU.
    ///
    /// Data used very often will end up closer to the beginning of the GPU memory over time, and
    /// will have fewer move operations applied to it as a result.
    ///
    /// The `lru` parameter must be sorted.
    ///
    /// Pending copies must have been reset before calling this.
    pub fn compact_cache(&mut self, timestamp: usize) {
        // Remove expired items from the LRU cache
        self.items.retain(|item| item.expiry > timestamp);

        // "Forget about" GPU memory regions occupied by unused data
        let lru = &self.items;
        self.usage
            .retain(|(_, key)| lru.binary_search_by(|probe| probe.key.cmp(&key)).is_ok());

        // We only need to compact the memory in the region preceding the dirty region, because that
        // data will be uploaded and used during this compilation - we will defer that region to the
        // next compilation
        let mut start = 0;
        let end = self.pending_write.as_ref().map_or_else(
            || {
                self.usage
                    .last()
                    .map_or(0, |(offset, key)| *offset + key.stride())
            },
            |dirty| dirty.start,
        );

        // Walk through the GPU memory in order, moving items back to the empty region and as we go
        for (offset, key) in &mut self.usage {
            let stride = key.stride();

            // Early out if we have exceeded the non-dirty region
            if *offset >= end {
                break;
            }

            // Skip items which should not be moved
            if start == *offset {
                start += stride;
                continue;
            }

            // Move this item back to the beginning of the empty region
            if let Some(range) = self.pending_copies.last_mut() {
                if range.src.end == *offset {
                    // The last pending copy will be expanded to include this key
                    range.src.end += stride;
                } else {
                    self.pending_copies.push(CopyRange {
                        dst: start,
                        src: *offset..*offset + stride,
                    });
                }
            } else {
                self.pending_copies.push(CopyRange {
                    dst: start,
                    src: *offset..*offset + stride,
                });
            }

            // Update the LRU item for this key
            let idx = self
                .items
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            self.items[idx].offset = start;
            *offset = start;

            start += stride;
        }
    }

    /// Returns the byte length of this cache.
    pub fn len(&self) -> u64 {
        self.usage
            .last()
            .map_or(0, |(offset, key)| offset + key.stride())
    }

    // TODO: Better name!
    pub fn reset(&mut self) {
        self.allocation.previous = None;
        self.pending_copies.clear();
        self.pending_write = None;
    }
}

/// Individual item of a least-recently-used cache vector. Allows tracking the usage of a key which
/// lives at some memory offset.
pub struct Lru<Key> {
    pub expiry: usize,
    pub key: Key,
    pub offset: u64,
}

pub trait Stride {
    fn stride(&self) -> u64;
}

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{gpu::Gpu, ptr::RcK},
    };

    #[test]
    fn dirty_data_compacts() {
        impl Stride for char {
            fn stride(&self) -> u64 {
                3 // bytes
            }
        }

        // Gpu::<RcK>::offscreen();

        // let mut pool = Pool::<RcK>::default();
        // let mut data = LruCache::<char, RcK>::from(unsafe {
        //     pool.data(
        //         #[cfg(feature = "debug-names")]
        //         "my data",
        //         1024,
        //     )
        // });

        // // dirty data usage retains `a` due to timestamp (4) not exceeding expiry (5)
        // pool.lru_timestamp = 4;
        // data.usage.push((0, 'a'));
        // data.compact_cache(&mut [Lru::new('a', 0, 5)]);
        // assert_eq!(data.usage.len(), 1);

        // // `a` is dropped after timestamp (5) equals expiry (5)
        // pool.lru_timestamp = 5;
        // data.compact_cache(&mut [Lru::new('a', 0, 5)]);
        // assert_eq!(data.usage.len(), 0);

        // // `b` is dropped after timestamp (6) exceeds expiry (5)
        // pool.lru_timestamp = 6;
        // data.usage.push((0, 'b'));
        // data.compact_cache(&mut [Lru::new('b', 0, 5)]);
        // assert_eq!(data.usage.len(), 0);

        // // `c` and `d` are compacted
        // pool.lru_timestamp = 2;
        // data.usage.push((3, 'c'));
        // data.usage.push((6, 'd'));
        // let mut lru = vec![Lru::new('c', 3, 5), Lru::new('d', 6, 5)];
        // data.compact_cache(&mut lru);
        // assert_eq!(lru.len(), 2);
        // assert_eq!(lru[0].key, 'c');
        // assert_eq!(lru[0].offset, 0);
        // assert_eq!(lru[1].key, 'd');
        // assert_eq!(lru[1].offset, 3);
        // assert_eq!(data.pending_copies.len(), 1);
        // assert_eq!(data.pending_copies[0].src, 3..9);
        // assert_eq!(data.pending_copies[0].dst, 0);

        // data.reset();
        // data.usage.clear();

        // // `e` and `f` are compacted
        // pool.lru_timestamp = 2;
        // data.usage.push((3, 'e'));
        // data.usage.push((9, 'f'));
        // let mut lru = vec![Lru::new('e', 3, 5), Lru::new('f', 9, 5)];
        // data.compact_cache(&mut lru);
        // assert_eq!(lru.len(), 2);
        // assert_eq!(lru[0].key, 'e');
        // assert_eq!(lru[0].offset, 0);
        // assert_eq!(lru[1].key, 'f');
        // assert_eq!(lru[1].offset, 3);
        // assert_eq!(data.pending_copies.len(), 2);
        // assert_eq!(data.pending_copies[0].src, 3..6);
        // assert_eq!(data.pending_copies[0].dst, 0);
        // assert_eq!(data.pending_copies[1].src, 9..12);
        // assert_eq!(data.pending_copies[1].dst, 3);
    }
}
