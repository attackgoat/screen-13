//! A collection of operation implementations used to fulfill the Render API.

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
    fn reset(&mut self) {
        self.pending_copies.clear();
        self.pending_write = None;
    }
}

impl<T, P> From<Lease<Data, P>> for DirtyData<T, P>
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

struct DirtyLruData<Key, P>
where
    P: SharedPointerKind,
{
    buf: Option<DirtyData<Key, P>>,
    lru: Vec<Lru<Key>>,
}

impl<K, P> DirtyLruData<K, P>
where
    P: SharedPointerKind,
{
    fn step(&mut self) {
        if let Some(buf) = self.buf.as_mut() {
            buf.reset();
        }

        // TODO: This should keep a 'frame' value per item and just increment a single 'age' value,
        // O(1) not O(N)!
        for item in self.lru.iter_mut() {
            item.recently_used = item.recently_used.saturating_sub(1);
        }
    }
}

// #[derive(Default)] did not work due to Key being unconstrained
impl<Key, P> Default for DirtyLruData<Key, P>
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
    key: T,
    offset: u64,
    recently_used: usize,
}

impl<T> Lru<T> {
    fn new(key: T, offset: u64, lru_threshold: usize) -> Self {
        Self {
            key,
            offset,
            recently_used: lru_threshold,
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
