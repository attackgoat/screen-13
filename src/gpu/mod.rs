//! Contains the heart and soul of _Screen 13_.
//!
//! This module does the heavy-lifting work in this library and is made from a number of
//! internal-only modules which support the functionality described here.
//!
//! ## Note About Operations
//!
//! All of the things `Render` allows are recorded internally using `Op` implementations which are
//! themselves just containers of graphics API resources and the commands to work on them.
//!
//! I think this structure is really flexible and clear and might live for a few good versions at
//! least.
//!
//! _NOTE:_ `Op` implementations should follow a few rules for sanity:
//! - Use a `Pool` instance to acquire new resources
//! - `new(...) -> Self`: function initializes with minimum settings
//! - `with_X(&mut self, ...) -> &mut Self`: Use the mutable borrow builder pattern for options/extras
//! - `record(&mut self, ...)`: function does three things:
//!   1. Initialize final settings/pipelines
//!   1. Write descriptors
//!   1. Submit all commands
//! - `submit_X(&self)`: functions which contain minimal "if" cases and logic; simple calls only.
//!
//! ## Note About `def`
//!
//! Internally _Screen 13_ uses pre-defined render passes and pipelines to function.
//!
//! All of these definitions exist in the `def` module and are *great*, but inflexible. They only
//! do what I programmed them to do. Lame.
//!
//! Instead, these definitions should be built and cached at runtime based on the operation which
//! have been recorded. There are follow-on things such as shaders to handle and so I'm not ready
//! for this change yet.
//!
//! Similiarly, the handling of command buffers and submission queues is currently on a
//! per-operation basis which is very limiting. We should keep running command buffers and only
//! close them when the graph of operations says so.

pub mod clear {
    //! Types for clearing textures with a configurable color.

    pub use super::op::clear::ClearOp;
}

pub mod copy {
    //! Types for copying textures with configurable source and destination coordinates.

    pub use super::op::copy::CopyOp;
}

pub mod draw {
    //! Types for drawing user-specified models and lights onto textures.

    pub use super::op::draw::{
        Command as Draw, DrawOp, LineCommand, Material, Mesh, ModelCommand, PointLightCommand,
        RectLightCommand, Skydome, SpotlightCommand, SunlightCommand,
    };
}

pub mod encode {
    //! Types for encoding textures into the `.jpg` or `.png` file formats.

    pub use super::op::encode::EncodeOp;
}

pub mod text {
    //! Types for writing text onto textures using stylized fonts.

    pub use super::op::text::{BitmapFont, Command as Text, ScalableFont, TextOp};
}

pub mod gradient {
    //! Types for filling textures with linear and radial gradients.

    pub use super::op::gradient::GradientOp;
}

pub mod write {
    //! Types for pasting/splatting textures with configurable source and destination transforms.

    pub use super::op::write::{Command as Write, Mode as WriteMode, WriteOp};
}

mod data;
mod def;
mod driver;
mod model;
mod op;
mod pool;
mod render;
mod swapchain;
mod texture;

#[rustfmt::skip]
mod spirv {
    include!(concat!(env!("OUT_DIR"), "/spirv/mod.rs"));
}

pub use self::{
    def::vertex,
    model::{MeshFilter, Model, Pose},
    op::bitmap::Bitmap,
    render::Render,
    texture::Texture,
};

pub(crate) use self::{op::Op, pool::Pool, swapchain::Swapchain};

use {
    self::{
        data::{Data, Mapping},
        driver::{Image2d, Surface},
        op::{
            bitmap::BitmapOp,
            text::{BitmapFont, ScalableFont},
        },
        pool::{Lease, PoolRef},
        vertex::Vertex,
    },
    crate::{
        math::Extent,
        pak::{
            id::{AnimationId, BitmapId, ModelId},
            model::Mesh,
            BitmapBuf, BitmapFormat, IndexType, Pak,
        },
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    fontdue::Font,
    gfx_hal::{
        adapter::{Adapter, DeviceType, MemoryProperties, PhysicalDevice},
        buffer::Usage,
        device::Device,
        memory::{HeapFlags, Properties},
        queue::{QueueFamily, QueueFamilyId, QueueGroup},
        window::Surface as _,
        Backend, Features, Instance as _, MemoryTypeId,
    },
    gfx_impl::{Backend as _Backend, Instance},
    num_traits::Num,
    std::{
        cell::RefCell,
        cmp::Ordering,
        fmt::Debug,
        io::{Read, Seek},
        mem::MaybeUninit,
        sync::Once,
    },
    winit::window::Window,
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

static mut ADAPTER: MaybeUninit<Adapter<_Backend>> = MaybeUninit::uninit();
static mut INIT: Once = Once::new();
static mut INSTANCE: MaybeUninit<Instance> = MaybeUninit::uninit();
static mut DEVICE: MaybeUninit<<_Backend as Backend>::Device> = MaybeUninit::uninit();
static mut MEM_PROPS: MaybeUninit<MemoryProperties> = MaybeUninit::uninit();
static mut QUEUE_GROUP: MaybeUninit<QueueGroup<_Backend>> = MaybeUninit::uninit();

/// Two-dimensional rendering result.
pub type Texture2d = Texture<Image2d>;

type LoadCache<P> = RefCell<Pool<P>>;

/// Rounds down a multiple of atom; panics if atom is zero
fn align_down<N: Copy + Num>(size: N, atom: N) -> N {
    size - size % atom
}

/// Rounds up to a multiple of atom; panics if either parameter is zero
fn align_up<N: Copy + Num>(size: N, atom: N) -> N {
    (size - <N>::one()) - (size - <N>::one()) % atom + atom
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn adapter() -> &'static Adapter<_Backend> {
    &*ADAPTER.as_ptr()
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn device() -> &'static <_Backend as Backend>::Device {
    &*DEVICE.as_ptr()
}

/// 💀 Extremely unsafe - call *ONLY* once per process!
unsafe fn init_gfx_hal() {
    const ENGINE: &str = "attackgoat/screen-13";
    const VERSION: u32 = 1;

    // Initialize the GFX-HAL library
    INSTANCE
        .as_mut_ptr()
        .write(Instance::create(ENGINE, VERSION).expect("Unable to create GFX-HAL instance"));

    let mut adapters = instance().enumerate_adapters();
    let adapter = if adapters.is_empty() {
        panic!("Unable to find GFX-HAL adapter");
    } else {
        // Prefer adapters by type in this order ...
        #[cfg(not(feature = "low-power"))]
        let type_rank = |ty: &DeviceType| -> u8 {
            match ty {
                DeviceType::DiscreteGpu => 0,
                DeviceType::IntegratedGpu => 1,
                DeviceType::VirtualGpu => 2,
                DeviceType::Cpu => 3,
                DeviceType::Other => 4,
            }
        };

        // ... this order when in low-power mode.
        #[cfg(feature = "low-power")]
        let type_rank = |ty: &DeviceType| -> u8 {
            match ty {
                DeviceType::IntegratedGpu => 0,
                DeviceType::Cpu => 1,
                DeviceType::VirtualGpu => 2,
                DeviceType::DiscreteGpu => 3,
                DeviceType::Other => 4,
            }
        };

        // Prefer adapters with the most memory
        let device_mem = |device: &<_Backend as Backend>::PhysicalDevice| -> u64 {
            device
                .memory_properties()
                .memory_heaps
                .iter()
                .filter(|heap| heap.flags.contains(HeapFlags::DEVICE_LOCAL))
                .map(|heap| heap.size)
                .sum()
        };

        adapters.sort_unstable_by(|a, b| {
            // 1. Best type of device for the current feature set (Default = performance)
            let a_type_rank = type_rank(&a.info.device_type);
            let b_type_rank = type_rank(&b.info.device_type);
            match a_type_rank.cmp(&b_type_rank) {
                Ordering::Equal => (),
                ne => return ne,
            }

            // 2. Most on-device memory
            let a_mem = device_mem(&a.physical_device);
            let b_mem = device_mem(&b.physical_device);
            match b_mem.cmp(&a_mem) {
                Ordering::Equal => (),
                ne => return ne,
            }

            // Fallback to device PCI ID (basically random, but always the same choice for a given
            // machine)
            a.info.device.cmp(&b.info.device)
        });

        let adapter = adapters.remove(0);

        info!(
            "Adapter #1: {} {:?} [Total Memory = {} GiB]",
            &adapter.info.name,
            adapter.info.device_type,
            device_mem(&adapter.physical_device) / 1024 / 1024 / 1024
        );

        for (idx, adapter) in adapters.iter().enumerate() {
            debug!(
                "Adapter #{}: {} {:?}",
                idx + 2,
                &adapter.info.name,
                adapter.info.device_type
            );
        }

        adapter
    };

    ADAPTER.as_mut_ptr().write(adapter);
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn instance() -> &'static Instance {
    &*INSTANCE.as_ptr()
}

/// Very unsafe - call *ONLY* after init!
unsafe fn mem_ty(mask: u32, props: Properties) -> Option<MemoryTypeId> {
    //debug!("type_mask={} properties={:?}", type_mask, properties);
    (*MEM_PROPS.as_ptr())
        .memory_types
        .iter()
        .enumerate()
        .position(|(idx, mem_ty)| {
            //debug!("Mem ID {} type={:?}", id, mem_type);
            // type_mask is a bit field where each bit represents a memory type. If the bit is set
            // to 1 it means we can use that type for our buffer. So this code finds the first
            // memory type that has a `1` (or, is allowed), and is visible to the CPU.
            mask & (1 << idx) != 0 && mem_ty.properties.contains(props)
        })
        .map(MemoryTypeId)
}

unsafe fn open_adapter(adapter: &Adapter<_Backend>, queue: &<_Backend as Backend>::QueueFamily) {
    info!(
        "Adapter: {} ({:?})",
        &adapter.info.name, adapter.info.device_type
    );

    let mut gpu = adapter
        .physical_device
        .open(&[(queue, &[1.0])], Features::empty())
        .expect("Unable to open GFX-HAL device");
    DEVICE.as_mut_ptr().write(gpu.device);
    MEM_PROPS
        .as_mut_ptr()
        .write(adapter.physical_device.memory_properties());
    QUEUE_GROUP.as_mut_ptr().write(
        gpu.queue_groups
            .pop()
            .expect("Unable to find GFX-HAL queue"),
    );
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn queue_family() -> QueueFamilyId {
    (*QUEUE_GROUP.as_ptr()).family
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn queue_mut() -> &'static mut <_Backend as Backend>::Queue {
    // TODO: MUTEX!

    &mut (*QUEUE_GROUP.as_mut_ptr()).queues[0]
}

/// Indicates the provided data was bad.
#[derive(Debug)]
pub struct BadData;

/// Specifies a method for combining two images using a mathmatical formula.
#[cfg(feature = "blend-modes")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlendMode {
    /// Blend formula: a + b
    Add,

    /// Blend formula:
    AlphaAdd,

    /// Blend formula: 1 - (1 - a) / b
    ColorBurn,

    /// Blend formula: a / (1 - b)
    ColorDodge,

    /// Blend formula:
    Color,

    /// Blend formula: min(a, b)
    Darken,

    /// Blend formula: min(a, b) (_per-component_)
    DarkerColor,

    /// Blend formula: abs(a - b)
    Difference,

    /// Blend formula: a / b
    Divide,

    /// Blend formula: a + b - 2 * a * b
    Exclusion,

    /// Blend formula: hard_light(a, b) (_per-component_)
    ///
    /// Where:
    /// - hard_light(a, b) = if b < 0.5 {
    ///                          2 * a * b
    ///                      } else {
    ///                          1 - 2 * (1 - a) * (1 - b)
    ///                      }
    HardLight,

    // TODO: It looks like this formula is correct and hard light and overlay are wrong or the
    // other way around?
    /// Blend formula: hard_mix(a, b) (_per-component_)
    ///
    /// Where:
    /// - hard_mix(a, b) = if b < 0.5 {
    ///                        2 * a * b
    ///                    } else {
    ///                        1 - 2 * (1 - a) * (1 - b)
    ///                    }
    HardMix,

    /// Blend formula: a + b - 1
    LinearBurn,

    /// Blend formula: a * b
    Multiply,

    /// Blend formula: b
    ///
    /// _NOTE:_ This is the default blend mode.
    Normal,

    /// Blend formula: overlay(a, b) (_per-component_)
    ///
    /// Where:
    /// - overlay(a, b) = if b < 0.5 {
    ///                       2 * a * b
    ///                   } else {
    ///                       1 - 2 * (1 - a) * (1 - b)
    ///                   }
    Overlay,

    /// Blend formula: 1 - (1 - a) * (1 - b)
    Screen,

    /// Blend formula: a - b
    Subtract,

    /// Blend formula: vivid_light(a, b) (_per-component_)
    ///
    /// Where:
    /// - vivid_light(a, b) = if b < 0.5 {
    ///                           1 - (1 - a) / b
    ///                       } else {
    ///                           a / (1 - b)
    ///                       }
    VividLight,
}

#[cfg(feature = "blend-modes")]
impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

// TODO: Make this drainable? Set a drain level for incoming pools and drain current ones?
/// An opaque cache of graphics API handles and resources.
///
/// For optimal performance, `Cache` instances should remain as owned values for at least three
/// hardware frames after their last use.
///
/// _NOTE:_ Program execution will halt for a few milliseconds after `Cache` types with active
/// internal operations are dropped.
pub struct Cache<P>
where
    P: 'static + SharedPointerKind,
{
    lru_threshold: usize,
    lru_timestamp: RefCell<usize>,
    pool: PoolRef<Pool<P>, P>,
}

impl<P> Default for Cache<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            lru_threshold: Self::DEFAULT_LRU_THRESHOLD,
            lru_timestamp: RefCell::new(0),
            pool: Default::default(),
        }
    }
}

impl<P> Cache<P>
where
    P: SharedPointerKind,
{
    /// The default number of frames which elapse before a cache item is considered obsolete.
    pub const DEFAULT_LRU_THRESHOLD: usize = 8;

    // TODO: Automatically call these functions on OOM so client doesn't even know?
    /// Allows you to remove unused resources from the cache.
    ///
    /// THIS API IS NOT IMPLEMENTED YET - Not sure about the final form yet.
    ///
    /// **_NOTE:_** _Screen 13_ will automatically drain unused resources.
    pub fn drain(&self) -> ! {
        todo!();
    }

    /// Returns the number of frames which elapse before a cache item is considered obsolete.
    ///
    /// Internal resources which have not been used within this number of frames will be reclaimed
    /// for reuse with other operations.
    pub fn lru_threshold(&self) -> usize {
        self.lru_threshold
    }

    /// Sets the number of frames which elapse before a cache item is considered obsolete.
    ///
    /// Internal resources which have not been used within this number of frames will be reclaimed
    /// for reuse with other operations
    ///
    /// **_NOTE:_** Higher numbers such as `10` will use more memory but have less thrashing than
    /// lower numbers, such as `1`.
    pub fn set_lru_threshold(&mut self, value: usize) {
        self.lru_threshold = value;
    }
}

/// Allows you to load resources and begin rendering operations.
pub struct Gpu<P>
where
    P: 'static + SharedPointerKind,
{
    loads: LoadCache<P>,
    renders: Cache<P>,
}

impl<P> Gpu<P>
where
    P: SharedPointerKind,
{
    pub(super) unsafe fn new(
        window: &Window,
        dims: Extent,
        swapchain_len: u32,
    ) -> (Self, Swapchain) {
        let mut surface = None;
        INIT.call_once(|| {
            init_gfx_hal();

            // Window mode requires a presentation surface (we check for support here)
            let surface_instance = Surface::new(window).expect("Unable to create GFX-HAL surface");
            let queue = adapter()
                .queue_families
                .iter()
                .find(|family| {
                    let ty = family.queue_type();

                    surface_instance.supports_queue_family(family)
                        && ty.supports_compute()
                        && ty.supports_graphics()
                        && ty.supports_transfer()
                })
                .expect("Unable to find GFX-HAL queue");

            info!(
                "Adapter: {} ({:?})",
                &adapter().info.name,
                adapter().info.device_type
            );

            surface = Some(surface_instance);

            open_adapter(adapter(), queue);
        });

        let gpu = Self {
            loads: Default::default(),
            renders: Default::default(),
        };
        let swapchain = Swapchain::new(surface.take().unwrap(), dims, swapchain_len);

        (gpu, swapchain)
    }

    /// Creates a `Gpu` for off-screen or headless use.
    pub fn offscreen() -> Self {
        unsafe {
            INIT.call_once(|| {
                init_gfx_hal();

                // Window mode requires a presentation surface (we check for support here)
                let queue = adapter()
                    .queue_families
                    .iter()
                    .find(|family| {
                        let ty = family.queue_type();

                        ty.supports_compute() && ty.supports_graphics() && ty.supports_transfer()
                    })
                    .expect("Unable to find GFX-HAL queue");

                open_adapter(adapter(), queue);
            });
        }

        Self {
            loads: Default::default(),
            renders: Default::default(),
        }
    }

    /// Loads a bitmap at runtime from the given data.
    ///
    /// `width` is specified in whole pixels.
    ///
    /// Pixel data must be specified as `R, [G, [B, [A]]]` bytes eaccording to `fmt`. The first
    /// index is the top left and the data proceeds left-to-right and row-by-row, top-to-bottom.
    ///
    /// **_NOTE:_** Each row must have `width` number of pixels. To load padded data use
    /// `[load_bitmap_with_stride]`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn __() {
    /// # use screen_13::prelude_rc::*;
    /// # let gpu = Gpu::offscreen();
    /// # let height = 32;
    /// let mut pixels = vec![];
    /// for y in 0..height {
    ///     for x in 0..32 {
    ///         pixels.push(0x00); // 🔴
    ///         pixels.push(0x80); // 🟢 These values make teal
    ///         pixels.push(0x80); // 🔵
    ///     }
    /// }
    ///
    /// let bitmap = gpu.load_bitmap(BitmapFormat::Rgb, 32, pixels);
    /// # }
    /// ```
    pub fn load_bitmap<I>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        fmt: BitmapFormat,
        width: u16,
        pixels: I,
    ) -> Shared<Bitmap<P>, P>
    where
        I: IntoIterator<Item = u8>,
    {
        self.load_bitmap_with_stride(
            #[cfg(feature = "debug-names")]
            name,
            fmt,
            width,
            pixels,
            width,
        )
    }

    /// Loads a bitmap at runtime from the given data, with a configurable row padding.
    ///
    /// `width` is specified in whole pixels.
    ///
    /// `stride` is specified in whole pixels and must be equal to or greater than `width`.
    ///
    /// Pixel data must be specified as `R, [G, [B, [A]]]` bytes eaccording to `fmt`. The first
    /// index is the top left and the data proceeds left-to-right and row-by-row, top-to-bottom.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn __() {
    /// # use screen_13::prelude_rc::*;
    /// # let gpu = Gpu::offscreen();
    /// # let height = 32;
    /// let mut pixels = vec![];
    /// for y in 0..height {
    ///     for x in 0..27 {
    ///         pixels.push(0x00); // 🔴
    ///         pixels.push(0x80); // 🟢 These values make teal
    ///         pixels.push(0x80); // 🔵
    ///     }
    ///
    ///     // Some libraries provide images that might be padded something like this:
    ///     // (This adds enough padding to make our stride into 32)
    ///     for x in 0..15 {
    ///         pixels.push(Default::default());
    ///     }
    /// }
    ///
    /// let bitmap = gpu.load_bitmap_with_stride(BitmapFormat::Rgb, 27, pixels, 32);
    /// # }
    /// ```
    pub fn load_bitmap_with_stride<I>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        fmt: BitmapFormat,
        width: u16,
        pixels: I,
        row_stride: u16,
    ) -> Shared<Bitmap<P>, P>
    where
        I: IntoIterator<Item = u8>,
    {
        debug_assert!(row_stride >= width);

        let bitmap = BitmapBuf::new(fmt, width, pixels.into_iter().collect());
        let mut pool = self.loads.borrow_mut();

        Shared::new(unsafe {
            BitmapOp::new(
                #[cfg(feature = "debug-names")]
                name,
                &mut pool,
                &bitmap,
            )
            .record()
        })
    }

    // TODO: Figure out what signature we want here, provide an iter of bitmap arrays plus def?
    /// Loads a bitmapped font at runtime from the given data.
    ///
    ///
    pub fn load_bitmap_font(&self) -> Result<Shared<BitmapFont<P>, P>, BadData> {
        todo!()
    }

    /// Loads an indexed model at runtime from the given data.
    ///
    ///
    pub fn load_indexed_model<I, Iv, M, V>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        meshes: M,
        _indices: I,
        _vertices: Iv,
    ) -> Result<Shared<Model<P>, P>, BadData>
    where
        M: IntoIterator<Item = Mesh>,
        I: IntoIterator<Item = u32>,
        Iv: IntoIterator<Item = V>,
        V: Copy + Into<Vertex>,
    {
        unsafe {
            let meshes = meshes.into_iter().collect::<Vec<_>>();
            // let indices = indices.into_iter().collect::<Vec<_>>();
            // let vertices = vertices.into_iter().collect::<Vec<_>>();

            // Make sure the incoming meshes are valid
            for mesh in &meshes {
                if mesh.vertex_count() % 3 != 0 {
                    return Err(BadData);
                }
            }

            let mut pool = self.loads.borrow_mut();

            let idx_buf_len = 0;
            let idx_buf = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                idx_buf_len,
                Usage::INDEX | Usage::STORAGE,
            );

            let vertex_buf_len = 0;
            let vertex_buf = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                vertex_buf_len,
                Usage::VERTEX | Usage::STORAGE,
            );

            let staging_buf_len = 0;
            let staging_buf = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                staging_buf_len,
                Usage::VERTEX | Usage::STORAGE,
            );

            let write_mask_len = 0;
            let write_mask = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                write_mask_len,
                Usage::STORAGE,
            );

            Ok(Shared::new(Model::new(
                meshes,
                IndexType::U32,
                (idx_buf, idx_buf_len),
                (vertex_buf, vertex_buf_len),
                (staging_buf, staging_buf_len, write_mask),
            )))
        }
    }

    /// Loads a regular model at runtime from the given data.
    ///
    ///
    pub fn load_model<Im, Iv, M, V>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        meshes: Im,
        vertices: Iv,
    ) -> Result<Shared<Model<P>, P>, BadData>
    where
        Im: IntoIterator<Item = M>,
        Iv: IntoIterator<Item = V>,
        M: Into<Mesh>,
        V: Copy + Into<Vertex>,
    {
        let mut meshes = meshes
            .into_iter()
            .map(|mesh| mesh.into())
            .collect::<Vec<_>>();
        let vertices = vertices.into_iter().collect::<Vec<_>>();
        let mut indices = vec![];

        // Add index data to the meshes (and build a buffer)
        let mut base_vertex = 0;
        for mesh in &mut meshes {
            let base_idx = indices.len();
            let mut cache: Vec<Vertex> = vec![];
            let vertex_count = mesh.vertex_count() as usize;

            // First we index the vertices ...
            for idx in base_vertex..base_vertex + vertex_count {
                let vertex = if let Some(vertex) = vertices.get(idx) {
                    (*vertex).into()
                } else {
                    return Err(BadData);
                };

                debug_assert!(vertex.is_finite());

                if let Err(idx) = cache.binary_search_by(|probe| probe.cmp(&vertex)) {
                    cache.insert(idx, vertex);
                }
            }

            // ... and then we push all the indices into the buffer
            for idx in base_vertex..base_vertex + vertex_count {
                let vertex = vertices.get(idx).unwrap();
                let vertex = (*vertex).into();
                let idx = cache.binary_search_by(|probe| probe.cmp(&vertex)).unwrap();
                indices.push(idx as _);
            }

            mesh.indices = base_idx as u32..(base_idx + indices.len()) as u32;
            base_vertex += vertex_count;
        }

        self.load_indexed_model(
            #[cfg(feature = "debug-names")]
            name,
            meshes,
            indices,
            vertices,
        )
    }

    /// Loads a scalable font at runtime from the given `fontdue::Font`.
    pub fn load_scalable_font(&self, font: Font) -> Shared<ScalableFont, P> {
        Shared::new(font.into())
    }

    // TODO: Finish this bit!
    /// Reads the `Animation` with the given id from the pak.
    pub fn read_animation<R>(
        &self,
        #[cfg(debug_assertions)] _name: &str,
        _pak: &mut Pak<R>,
        _id: AnimationId,
    ) -> usize
    where
        R: Read + Seek,
    {
        //let _pool = PoolRef::clone(&self.pool);
        //let _anim = pak.read_animation(id);
        // let indices = model.indices();
        // let index_buf_len = indices.len() as _;
        // let mut index_buf = pool.borrow_mut().data_usage(
        //     #[cfg(debug_assertions)]
        //     name,
        //     index_buf_len,
        //     Usage::INDEX,
        // );

        // {
        //     let mut mapped_range = index_buf.map_range_mut(0..index_buf_len).unwrap();
        //     mapped_range.copy_from_slice(&indices);
        //     Mapping::flush(&mut mapped_range).unwrap();
        // }

        // let vertices = model.vertices();
        // let vertex_buf_len = vertices.len() as _;
        // let mut vertex_buf = pool.borrow_mut().data_usage(
        //     #[cfg(debug_assertions)]
        //     name,
        //     vertex_buf_len,
        //     Usage::VERTEX,
        // );

        // {
        //     let mut mapped_range = vertex_buf.map_range_mut(0..vertex_buf_len).unwrap();
        //     mapped_range.copy_from_slice(&vertices);
        //     Mapping::flush(&mut mapped_range).unwrap();
        // }

        // let model = Model::new(
        //     model.meshes().map(Clone::clone).collect(),
        //     index_buf,
        //     vertex_buf,
        // );

        // ModelRef::new(model)
        todo!()
    }

    /// Reads the `Bitmap` with the given key from the pak.
    pub fn read_bitmap<K, R>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Shared<Bitmap<P>, P>
    where
        K: AsRef<str>,
        R: Read + Seek,
    {
        let id = pak.bitmap_id(key).unwrap();

        self.read_bitmap_with_id(
            #[cfg(feature = "debug-names")]
            name,
            pak,
            id,
        )
    }

    /// Reads the `Bitmap` with the given id from the pak.
    pub fn read_bitmap_with_id<R>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: BitmapId,
    ) -> Shared<Bitmap<P>, P>
    where
        R: Read + Seek,
    {
        let bitmap = pak.read_bitmap(id);
        let mut pool = self.loads.borrow_mut();

        Shared::new(unsafe {
            BitmapOp::new(
                #[cfg(feature = "debug-names")]
                name,
                &mut pool,
                &bitmap,
            )
            .record()
        })
    }

    /// Reads the `BitmapFont` with the given face from the pak.
    pub fn read_bitmap_font<F, R>(&self, pak: &mut Pak<R>, face: F) -> Shared<BitmapFont<P>, P>
    where
        F: AsRef<str>,
        R: Read + Seek,
    {
        #[cfg(debug_assertions)]
        debug!("Loading bitmap font `{}`", face.as_ref());

        Shared::new(BitmapFont::read(
            &mut self.loads.borrow_mut(),
            pak,
            face.as_ref(),
        ))
    }

    /// Reads the `Model` with the given key from the pak.
    pub fn read_model<K, R>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Shared<Model<P>, P>
    where
        K: AsRef<str>,
        R: Read + Seek,
    {
        let id = pak.model_id(key).unwrap();

        self.read_model_with_id(
            #[cfg(feature = "debug-names")]
            name,
            pak,
            id,
        )
    }

    /// Reads the `Model` with the given id from the pak.
    pub fn read_model_with_id<R>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: ModelId,
    ) -> Shared<Model<P>, P>
    where
        R: Read + Seek,
    {
        unsafe {
            let mut pool = self.loads.borrow_mut();
            let model = pak.read_model(id);

            // Create an index buffer
            let (idx_buf, idx_buf_len) = {
                let src = model.indices();
                let len = src.len() as _;
                let mut buf = pool.data_usage(
                    #[cfg(feature = "debug-names")]
                    name,
                    len,
                    Usage::INDEX | Usage::STORAGE,
                );

                // Fill the index buffer
                {
                    let mut mapped_range = buf.map_range_mut(0..len).unwrap();
                    mapped_range.copy_from_slice(src);
                    Mapping::flush(&mut mapped_range).unwrap();
                }

                (buf, len)
            };

            // Create a staging buffer (holds vertices before we calculate additional vertex attributes)
            let (staging_buf, staging_buf_len) = {
                let src = model.vertices();
                let len = src.len() as _;
                let mut buf = pool.data_usage(
                    #[cfg(feature = "debug-names")]
                    name,
                    len,
                    Usage::STORAGE,
                );

                // Fill the staging buffer
                {
                    let mut mapped_range = buf.map_range_mut(0..len).unwrap();
                    mapped_range.copy_from_slice(src);
                    Mapping::flush(&mut mapped_range).unwrap();
                }

                (buf, len)
            };

            // The write mask is the used during vertex attribute calculation
            let write_mask = {
                let src = model.write_mask();
                let len = src.len() as _;
                let mut buf = pool.data_usage(
                    #[cfg(feature = "debug-names")]
                    name,
                    len,
                    Usage::STORAGE,
                );

                // Fill the write mask buffer
                {
                    let mut mapped_range = buf.map_range_mut(0..len).unwrap();
                    mapped_range.copy_from_slice(src);
                    Mapping::flush(&mut mapped_range).unwrap();
                }

                buf
            };

            let idx_ty = model.idx_ty();
            let mut meshes = model.take_meshes();
            let mut vertex_buf_len = 0;
            for mesh in &mut meshes {
                let stride = if mesh.is_animated() { 80 } else { 48 };

                // We pad each mesh in the vertex buffer so that drawing is easier (no vertex
                // re-binds; but this requires that all vertices in the buffer have a compatible
                // alignment. Because we have static (12 floats/48 bytes) and animated (20 floats/
                // 80 bytes) vertices, we round up to 60 floats/240 bytes. This means any possible
                // boundary we try to draw at will start at some multiple of either the static
                // or animated vertices.
                vertex_buf_len += vertex_buf_len % 240;
                mesh.set_base_vertex((vertex_buf_len / stride) as _);

                // Account for the vertices, updating the base vertex
                vertex_buf_len += mesh.vertex_count() as u64 * stride;
            }

            // This is the real vertex buffer which will hold the calculated attributes
            let vertex_buf = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                vertex_buf_len,
                Usage::STORAGE | Usage::VERTEX,
            );

            Shared::new(Model::new(
                meshes,
                idx_ty,
                (idx_buf, idx_buf_len),
                (vertex_buf, vertex_buf_len),
                (staging_buf, staging_buf_len, write_mask),
            ))
        }
    }

    /// Reads the `ScalableFont` with the given face from the pak.
    pub fn read_scalable_font<F, R>(&self, pak: &mut Pak<R>, face: F) -> ScalableFont
    where
        F: AsRef<str>,
        R: Read + Seek,
    {
        ScalableFont::read(&mut self.loads.borrow_mut(), pak, face.as_ref())
    }

    /// Constructs a `Render` of the given dimensions.
    ///
    /// _NOTE:_ This function uses an internal cache.
    ///
    /// # Examples:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # let gpu = Gpu::offscreen();
    /// let dims = (128u32, 128u32);
    /// let mut frame = gpu.render(dims);
    /// ```
    pub fn render<D>(&self, #[cfg(feature = "debug-names")] name: &str, dims: D) -> Render<P>
    where
        D: Into<Extent>,
    {
        self.render_with_cache(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            &self.renders,
        )
    }

    /// Constructs a `Render` of the given dimensions, using the provided cache.
    ///
    /// # Examples:
    ///
    /// ```rust
    /// # use screen_13::prelude_rc::*;
    /// # let gpu = Gpu::offscreen();
    /// let dims = (128u32, 128u32);
    /// let cache = Default::default();
    /// let mut frame = gpu.render_with_cache(dims, &cache);
    /// ```
    pub fn render_with_cache<D>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        dims: D,
        cache: &Cache<P>,
    ) -> Render<P>
    where
        D: Into<Extent>,
    {
        // Pull a rendering pool from the cache or we create and lease a new one
        let mut pool = if let Some(pool) = cache.pool.borrow_mut().pop_back() {
            pool
        } else {
            debug!("Creating new render pool");
            Default::default()
        };

        // Increment the LRU timestamp and handle wrapping (Note that when expiry < timestamp no
        // caching will happen - this happens once every 2.26 years on a 32bit system or once every
        // 9.75 billion years on a 64bit system but dont worry it only lasts lru_threshold frames!)
        let mut lru_timestamp = cache.lru_timestamp.borrow_mut();
        let (timestamp, _) = lru_timestamp.overflowing_add(1);
        *lru_timestamp = timestamp;

        // Set the expiry timestamp for resources created during this render
        let (expiry, _) = timestamp.overflowing_add(cache.lru_threshold);
        pool.lru_expiry = expiry;

        // Access to this pool is leased to the render from this cache; when dropped it'll be
        // returned to the cache pool
        let pool = Lease::new(pool, &cache.pool);

        unsafe {
            Render::new(
                #[cfg(feature = "debug-names")]
                name,
                dims.into(),
                pool,
            )
        }
    }

    pub(crate) unsafe fn wait_idle(&self) {
        #[cfg(debug_assertions)]
        let started = Instant::now();

        // We are required to wait for the GPU to finish what we submitted before dropping the driver
        device().wait_idle().unwrap();

        #[cfg(debug_assertions)]
        {
            let elapsed = Instant::now() - started;
            debug!(
                "Wait for GPU idle took {}ms",
                elapsed.as_millis().to_formatted_string(&Locale::en)
            );
        }
    }
}

impl<P> Drop for Gpu<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        unsafe {
            self.wait_idle();
        }
    }
}

/// Masking is the process of modifying a target image alpha channel using a series of
/// blend-like operations.
///
/// TODO: This feature isn't fully implemented yet
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MaskMode {
    /// Mask formula: a + b
    Add,

    /// Mask formula: min(a, b)
    Darken,

    /// Mask formula: abs(a - b)
    Difference,

    /// Mask formula: a * b
    Intersect,

    /// Mask formula: max(a, b)
    Lighten,

    /// Mask formula: a - b
    Subtract,
}

impl Default for MaskMode {
    fn default() -> Self {
        Self::Add
    }
}

/// Matting blends two images (a into b) based on the features of a matte image.
///
/// TODO: This feature isn't fully implemented yet
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MatteMode {
    /// Matte formula: alpha = min(a, matte)
    ///                color = a * alpha;
    Alpha,

    /// Matte formula: alpha = min(a, 1 - matte)
    ///                color = a * alpha;
    AlphaInverted,

    /// Matte formula: gray = hsl-based gray function
    ///                alpha = min(a, gray(matte))
    ///                color = a * alpha;
    Luminance,

    /// Matte formula: gray = hsl-based gray function
    ///                alpha = min(a, 1 - gray(matte))
    ///                color = a * alpha;
    LuminanceInverted,
}

impl Default for MatteMode {
    fn default() -> Self {
        Self::Alpha
    }
}
