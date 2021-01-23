//! A collection of operation implementations used to fulfill the Render API.

// TODO: Add automatic instancing based on lru data to secondary uses? draw and text and use now,
// write should definitely have this too.

pub mod bitmap;
pub mod clear;
pub mod copy;
pub mod draw;
pub mod encode;
pub mod gradient;
pub mod text;
pub mod write;

use {
    super::{data::CopyRange, Data, Lease, Pool},
    a_r_c_h_e_r_y::SharedPointerKind,
    std::any::Any,
    std::ops::Range,
};

// Always ask for a bigger cache capacity than needed; it reduces the need to completely replace
// the existing cache and then have to copy all the old data over.
const CACHE_CAPACITY_FACTOR: f32 = 2.0;

/// Used to keep track of data allocated during compilation and also the previous value which we
/// will copy over during the drawing operation.
struct Allocation<T> {
    current: T,
    previous: Option<(T, u64)>,
}

/// Extends the data type so we can track which portions require updates. Does not teach an entire
/// city full of people that dancing is the best thing there is.
struct DirtyData<Key, P>
where
    P: SharedPointerKind,
{
    data: Allocation<Lease<Data, P>>,

    /// Segments of gpu memory which must be "compacted" (read: copied) within the gpu.
    pending_copies: Vec<CopyRange>,

    /// This range, if present, is the portion that needs to be written from cpu to gpu.
    pending_write: Option<Range<u64>>,

    /// Memory usage on the gpu, sorted by the first field which is the offset.
    usage: Vec<(u64, Key)>,
}

impl<Key, P> DirtyData<Key, P>
where
    P: SharedPointerKind,
{
    /// Moves cache items into clumps so future items can be appended onto the end without needing
    /// to resize the cache buffer. As a side effect this causes dirty regions to be moved on the
    /// GPU.
    ///
    /// Geometry used very often will end up closer to the beginning of the GPU memory over time,
    /// and will have fewer move operations applied to it as a result.
    fn compact_cache(&mut self, lru: &mut [Lru<Key>], timestamp: usize)
    where
        Key: Ord + Stride,
    {
        let stride = Key::stride();

        // "Forget about" GPU memory regions occupied by unused data
        self.usage.retain(|(_, key)| {
            let idx = lru
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            timestamp >= lru[idx].expiry
        });

        // We only need to compact the memory in the region preceding the dirty region, because that geometry will
        // be uploaded and used during this compilation (draw) - we will defer that region to the next compilation
        let mut start = 0;
        let end = self.pending_write.as_ref().map_or_else(
            || self.usage.last().map_or(0, |(offset, _)| offset + stride),
            |dirty| dirty.start,
        );

        // Walk through the GPU memory in order, moving items back to the "empty" region and as we go
        for (offset, key) in &mut self.usage {
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
                if range.dst == start - stride && range.src.end == *offset - stride {
                    *range = CopyRange {
                        dst: range.dst,
                        src: range.src.start..*offset + stride,
                    };
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
            let idx = lru
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            lru[idx].offset = start;

            start += stride;
        }
    }

    fn reset(&mut self) {
        self.pending_copies.clear();
        self.pending_write = None;
    }
}

impl<Key, P> From<Lease<Data, P>> for DirtyData<Key, P>
where
    P: SharedPointerKind,
{
    fn from(val: Lease<Data, P>) -> Self {
        Self {
            data: Allocation {
                current: val,
                previous: None,
            },
            pending_copies: vec![],
            pending_write: None,
            usage: vec![],
        }
    }
}

struct DirtyLruData<T, P>
where
    P: SharedPointerKind,
{
    buf: Option<DirtyData<T, P>>,
    lru: Vec<Lru<T>>,
}

impl<P, T> DirtyLruData<T, P>
where
    P: SharedPointerKind,
{
    /// Allocates or re-allocates leased data of the given size.
    unsafe fn alloc_data(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        len: u64,
    ) where
        T: Stride,
    {
        #[cfg(feature = "debug-names")]
        if let Some(buf) = buf.as_mut() {
            buf.data.current.set_name(&name);
        }

        // Early-our if we do not need to resize the buffer
        if let Some(existing) = self.buf.as_ref() {
            if len <= existing.data.current.capacity() {
                return;
            }
        }

        #[cfg(debug_assertions)]
        {
            info!(
                "Reallocating {} to {}",
                self.buf
                    .as_ref()
                    .map_or(0, |buf| buf.data.current.capacity()),
                len
            );
        }

        // We over-allocate the requested capacity to prevent rapid reallocations
        let capacity = (len as f32 * CACHE_CAPACITY_FACTOR) as u64;
        let data = pool.data(
            #[cfg(feature = "debug-names")]
            &name,
            capacity,
        );

        if let Some(old_buf) = self.buf.replace(data.into()) {
            // Preserve the old data so that we can copy it directly over before drawing
            let old_buf_len = old_buf
                .usage
                .last()
                .map_or(0, |(offset, _)| offset + T::stride());
            let new_buf = &mut self.buf.as_mut().unwrap();
            new_buf.usage = old_buf.usage;
            new_buf.data.previous = Some((old_buf.data.current, old_buf_len));
        }
    }

    fn step(&mut self) {
        if let Some(buf) = self.buf.as_mut() {
            buf.reset();
        }

        // TODO: This should keep a 'frame' value per item and just increment a single 'age' value,
        // O(1) not O(N)!
        // for item in self.lru.iter_mut() {
        //     item.recently_used = item.recently_used.saturating_sub(1);
        // }


    }
}

// #[derive(Default)] did not work due to Key being unconstrained
impl<P, T> Default for DirtyLruData<T, P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            buf: None,
            lru: vec![],
        }
    }
}

/// Individual item of a least-recently-used cache vector. Allows tracking the usage of a key which
/// lives at some memory offset.
struct Lru<T> {
    expiry: usize,
    key: T,
    offset: u64,
}

impl<T> Lru<T> {
    fn new(key: T, offset: u64, expiry: usize) -> Self {
        Self {
            expiry,
            key,
            offset,
        }
    }
}

// TODO: `as_any_mut` and `take_pool` will only be used be ops which are part of the `Render`
// system. I should probably create a secondary trait for those bits. See todo!(..) in `Bitmap`.
pub trait Op<P>: Any
where
    P: SharedPointerKind,
{
    fn as_any_mut(&mut self) -> &mut dyn Any;

    unsafe fn is_complete(&self) -> bool;

    unsafe fn take_pool(&mut self) -> Lease<Pool<P>, P>; // TODO: This should become 'take_cmd'! and
                                                         // include cmd buf too

    unsafe fn wait(&self);
}

// TODO: All the places where we bind descriptor sets blindly allow the number of descriptors to be
// unbounded. Should work in groups beyond the limit so the API doesn't have to change.
// TODO: Like above, the places where we dispatch compute resources should probably also allow for
// batch-sized groups within device limits

trait Stride {
    fn stride() -> u64;
}
