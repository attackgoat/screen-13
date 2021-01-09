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
//! ## Note About `Rc`
//!
//! Screen 13 currently uses the `Rc` type in order to synchronize work between various operations.
//!
//! Unfortunately, this limits Screen 13 resources from being shared across threads. We could
//! simply replace those references with `Arc` and call it a day (that would work), but I'm not yet
//! sure that's a good pattern, it would introduce unessecery resource synchronization.
//!
//! I think as the API matures a better solution will become clearer, just not sure which types I
//! want to be `Send` yet. The driver really could be `Sync`.
//!
//! ## Note About `def`
//!
//! Internally Screen 13 uses pre-defined render passes and pipelines to function.
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
        Draw, DrawOp, LineCommand, Material, Mesh, ModelCommand, PointLightCommand,
        RectLightCommand, Skydome, SpotlightCommand, SunlightCommand,
    };
}

pub mod encode {
    //! Types for encoding textures into the `.jpg` or `.png` file formats.

    pub use super::op::encode::EncodeOp;
}

pub mod text {
    //! Types for writing text onto textures using stylized fonts.

    pub use super::op::font::{Font, FontOp};
}

pub mod gradient {
    //! Types for filling textures with linear and radial gradients.

    pub use super::op::gradient::GradientOp;
}

pub mod write {
    //! Types for pasting/splatting textures with configurable source and destination transforms.

    pub use super::op::write::{Mode as WriteMode, Write, WriteOp};
}

mod data;
mod def;
mod driver;
mod model;
mod op;
mod pool;
mod render;
mod spirv {
    include!(concat!(env!("OUT_DIR"), "/spirv/mod.rs"));
}
mod swapchain;
mod texture;

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
        op::{bitmap::BitmapOp, font::Font},
        pool::{Lease, PoolRef},
        vertex::Vertex,
    },
    crate::{
        math::Extent,
        pak::{
            id::{AnimationId, BitmapId, ModelId},
            model::Mesh,
            BitmapFormat, IndexType, Pak,
        },
        ptr::Shared,
    },
    archery::SharedPointerKind,
    gfx_hal::{
        adapter::{Adapter, MemoryProperties, PhysicalDevice},
        buffer::Usage,
        device::Device,
        memory::Properties,
        queue::{QueueFamily, QueueFamilyId, QueueGroup},
        window::Surface as _,
        Backend, Features, Instance as _, MemoryTypeId,
    },
    gfx_impl::{Backend as _Backend, Instance},
    num_traits::Num,
    std::{
        cell::RefCell,
        fmt::Debug,
        io::{Read, Seek},
        mem::MaybeUninit,
        rc::Rc,
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
pub type Texture2d = TextureRef<Image2d>;

// TODO: Replace with archery?
pub(crate) type TextureRef<I> = Rc<RefCell<Texture<I>>>;

type LoadCache<P> = RefCell<Pool<P>>;
type OpCache<P> = RefCell<Option<Vec<Box<dyn Op<P>>>>>;

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
unsafe fn device() -> &'static <_Backend as Backend>::Device {
    &*DEVICE.as_ptr()
}

/// 💀 Very unsafe - call *ONLY* once per process!
unsafe fn init_gfx_hal() {
    // Initialize the GFX-HAL library
    let engine = "attackgoat/screen-13";
    let version = 1;
    *INSTANCE.as_mut_ptr() =
        Instance::create(engine, version).expect("Unable to create GFX-HAL instance");

    let instance = &*INSTANCE.as_ptr();
    let mut adapters = instance.enumerate_adapters();
    *ADAPTER.as_mut_ptr() = if !adapters.is_empty() {
        adapters.remove(0)
    } else {
        panic!("Unable to find GFX-HAL adapter");
    };
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
    *DEVICE.as_mut_ptr() = gpu.device;
    *MEM_PROPS.as_mut_ptr() = adapter.physical_device.memory_properties();
    *QUEUE_GROUP.as_mut_ptr() = gpu
        .queue_groups
        .pop()
        .expect("Unable to find GFX-HAL queue");
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn physical_device() -> &'static <_Backend as Backend>::PhysicalDevice {
    &(*ADAPTER.as_ptr()).physical_device
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn queue_family() -> QueueFamilyId {
    (*QUEUE_GROUP.as_ptr()).family
}

/// Very unsafe - call *ONLY* after init!
#[inline]
unsafe fn queue_mut() -> &'static mut <_Backend as Backend>::CommandQueue {
    // TODO: MUTEX!

    &mut (*QUEUE_GROUP.as_mut_ptr()).queues[0]
}

/// Indicates the provided data was bad.
#[derive(Debug)]
pub struct BadData;

/// Specifies a method for combining two images using a mathmatical formula.
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

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

// TODO: Make this drainable?
/// An opaque cache of graphics API handles and resources.
///
/// For optimal performance, `Cache` instances should remain as owned values for at least three
/// hardware frames after their last use.
///
/// _NOTE:_ Program execution will halt for a few milliseconds after `Cache` types with active
/// internal operations are dropped.
#[derive(Default)]
pub struct Cache<P>(PoolRef<Pool<P>, P>)
where
    P: 'static + SharedPointerKind;

/// Allows you to load resources and begin rendering operations.
pub struct Gpu<P>
where
    P: 'static + SharedPointerKind,
{
    loads: LoadCache<P>,
    ops: OpCache<P>,
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
            let adapter = &*ADAPTER.as_ptr();
            let surface_instance = Surface::new(window).expect("Unable to create GFX-HAL surface");
            let queue = adapter
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
                &adapter.info.name, adapter.info.device_type
            );

            surface = Some(surface_instance);

            open_adapter(adapter, queue);
        });

        // let gpu = Self {
        //     loads: Default::default(),
        //     ops: Default::default(),
        //     renders: Default::default(),
        // };
        // let swapchain = Swapchain::new(surface.take().unwrap(), dims, swapchain_len);

        // (gpu, swapchain)
        todo!("DONT CHECKIN");
    }

    // TODO: Enable sharing between this and "on-screen"
    /// Creates a `Gpu` for off-screen or headless use.
    ///
    /// _NOTE_: Resources loaded or read from a `Gpu` created in headless or screen modes cannot be
    /// used with other instances, including of the same mode. This is a limitation only because
    /// the code to share the resources properly has not be started yet.
    pub fn offscreen() -> Self {
        unsafe {
            INIT.call_once(|| {
                init_gfx_hal();

                // Window mode requires a presentation surface (we check for support here)
                let adapter = &*ADAPTER.as_ptr();
                let queue = adapter
                    .queue_families
                    .iter()
                    .find(|family| {
                        let ty = family.queue_type();

                        ty.supports_compute() && ty.supports_graphics() && ty.supports_transfer()
                    })
                    .expect("Unable to find GFX-HAL queue");

                open_adapter(adapter, queue);
            });
        }

        // Self {
        //     loads: Default::default(),
        //     ops: Default::default(),
        //     renders: Default::default(),
        // }
        todo!("DONT CHECKIN");
    }

    /// Loads a bitmap at runtime from the given data.
    ///
    ///
    pub fn load_bitmap(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pixel_ty: BitmapFormat,
        pixels: &[u8],
        width: u32,
        stride: u32,
    ) -> Shared<Bitmap<P>, P> {
        #[cfg(feature = "debug-names")]
        let _ = name;
        let _ = pixel_ty;
        let _ = pixels;
        let _ = width;
        let _ = stride;

        todo!();
    }

    /// Loads an indexed model at runtime from the given data.
    ///
    ///
    pub fn load_indexed_model<
        M: IntoIterator<Item = Mesh>,
        I: IntoIterator<Item = u32>,
        V: IntoIterator<Item = VV>,
        VV: Copy + Into<Vertex>,
    >(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        meshes: M,
        _indices: I,
        _vertices: V,
    ) -> Result<Shared<Model<P>, P>, BadData> {
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
    pub fn load_model<
        IM: IntoIterator<Item = M>,
        IV: IntoIterator<Item = V>,
        M: Into<Mesh>,
        V: Copy + Into<Vertex>,
    >(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        meshes: IM,
        vertices: IV,
    ) -> Result<Shared<Model<P>, P>, BadData> {
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

    // TODO: Finish this bit!
    /// Reads the `Animation` with the given id from the pak.
    pub fn read_animation<R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] _name: &str,
        _pak: &mut Pak<R>,
        _id: AnimationId,
    ) -> () {
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
    pub fn read_bitmap<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Shared<Bitmap<P>, P> {
        let id = pak.bitmap_id(key).unwrap();

        self.read_bitmap_with_id(
            #[cfg(feature = "debug-names")]
            name,
            pak,
            id,
        )
    }

    /// Reads the `Bitmap` with the given id from the pak.
    pub fn read_bitmap_with_id<R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: BitmapId,
    ) -> Shared<Bitmap<P>, P> {
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

    /// Reads the `Font` with the given face from the pak.
    ///
    /// Only bitmapped fonts are supported.
    pub fn read_font<F: AsRef<str>, R: Read + Seek>(&self, pak: &mut Pak<R>, face: F) -> Font<P> {
        #[cfg(debug_assertions)]
        debug!("Loading font `{}`", face.as_ref());

        Font::load(&mut self.loads.borrow_mut(), pak, face.as_ref())
    }

    /// Reads the `Model` with the given key from the pak.
    pub fn read_model<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Shared<Model<P>, P> {
        let id = pak.model_id(key).unwrap();

        self.read_model_with_id(
            #[cfg(feature = "debug-names")]
            name,
            pak,
            id,
        )
    }

    /// Reads the `Model` with the given id from the pak.
    pub fn read_model_with_id<R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: ModelId,
    ) -> Shared<Model<P>, P> {
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

    /// Constructs a `Render` of the given dimensions.
    ///
    /// _NOTE:_ This function uses an internal cache.
    ///
    /// ## Examples:
    ///
    /// ```
    /// use screen_13::prelude_all::*;
    ///
    /// struct Foo;
    ///
    /// impl Screen for Foo {
    ///     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///         let frame = gpu.render(dims);
    ///
    ///         ...
    ///     }
    ///
    ///     ...
    /// }
    pub fn render<D: Into<Extent>>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        dims: D,
    ) -> Render<P> {
        self.render_with_cache(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            &self.renders,
        )
    }

    /// Constructs a `Render` of the given dimensions, using the provided cache.
    ///
    /// ## Examples:
    ///
    /// ```
    /// use screen_13::prelude_all::*;
    ///
    /// #[derive(Default)]
    /// struct Foo(Cache);
    ///
    /// impl Screen for Foo {
    ///     fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
    ///         let cache = &self.0;
    ///         let frame = gpu.render(dims, cache);
    ///
    ///         ...
    ///     }
    ///
    ///     ...
    /// }
    pub fn render_with_cache<D: Into<Extent>>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        dims: D,
        cache: &Cache<P>,
    ) -> Render<P> {
        // There may be pending operations from a previously resolved render; if so
        // we just stick them into the next render that goes out the door.
        let ops = if let Some(ops) = self.ops.borrow_mut().take() {
            ops
        } else {
            Default::default()
        };

        // Pull a rendering pool from the cache or we create and lease a new one
        let pool = if let Some(pool) = cache.0.borrow_mut().pop_back() {
            pool
        } else {
            debug!("Creating new render pool");
            Default::default()
        };
        let pool = Lease::new(pool, &cache.0);

        unsafe {
            Render::new(
                #[cfg(feature = "debug-names")]
                name,
                dims.into(),
                pool,
                ops,
            )
        }
    }

    /// Resolves a render into a texture which can be written to other renders.
    pub fn resolve(&self, render: Render<P>) -> Lease<Texture2d, P> {
        let (target, ops) = render.resolve();
        let mut cache = self.ops.borrow_mut();
        if let Some(cache) = cache.as_mut() {
            cache.extend(ops);
        } else {
            cache.replace(ops);
        }

        target
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
