mod compute;
mod data;

/// A collection of smart-pointer types used internally to operate the GFX-HAL API.
mod driver;

mod graphics;
mod model;

/// A collection of operation implementations used internally to fulfill the Render API.
mod op;

/// A collection of resource pool types used internally to cache GFX-HAL types.
mod pool;

mod render;
mod render_passes;
mod spirv {
    include!(concat!(env!("OUT_DIR"), "/spirv/mod.rs"));
}
mod swapchain;
mod texture;

pub use self::{
    model::{MeshFilter, Model, Pose},
    op::{Bitmap, Command, Font, Material, Write, WriteMode},
    pool::Pool,
    render::Render,
    swapchain::Swapchain,
    texture::Texture,
};

pub(crate) use self::{compute::Compute, driver::Driver, graphics::Graphics, op::Op};

use {
    self::{
        data::{Data, Mapping},
        driver::{Device, Image2d, Surface},
        op::BitmapOp,
        pool::{Lease, PoolRef},
    },
    crate::{
        math::Extent,
        pak::{AnimationId, BitmapId, IndexType, ModelId, Pak},
        Error,
    },
    gfx_hal::{
        adapter::Adapter, buffer::Usage, device::Device as _, format::Format, queue::QueueFamily,
        window::Surface as _, Instance as _,
    },
    gfx_impl::{Backend as _Backend, Instance},
    num_traits::Num,
    std::{
        cell::RefCell,
        fmt::Debug,
        io::{Read, Seek},
        rc::Rc,
    },
    winit::window::Window,
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

/// A two dimensional rendering result.
pub type Texture2d = TextureRef<Image2d>;

pub type BitmapRef = Rc<Bitmap>;
pub type ModelRef = Rc<Model>;

pub(crate) type TextureRef<I> = Rc<RefCell<Texture<I>>>;

type LoadCache = RefCell<Pool>;
type OpCache = RefCell<Option<Vec<Box<dyn Op>>>>;

/// Rounds down a multiple of atom; panics if atom is zero
fn align_down<N: Copy + Num>(size: N, atom: N) -> N {
    size - size % atom
}

/// Rounds up to a multiple of atom; panics if either parameter is zero
fn align_up<N: Copy + Num>(size: N, atom: N) -> N {
    (size - <N>::one()) - (size - <N>::one()) % atom + atom
}

fn create_instance() -> (Adapter<_Backend>, Instance) {
    let instance = Instance::create("attackgoat/screen-13", 1).unwrap();
    let mut adapters = instance.enumerate_adapters();
    if adapters.is_empty() {
        // TODO: Error::adapter
    }
    let adapter = adapters.remove(0);
    (adapter, instance)
}

// TODO: Different path for webgl and need this -> #[cfg(any(feature = "vulkan", feature = "metal"))]
fn create_surface(window: &Window) -> (Adapter<_Backend>, Surface) {
    let (adapter, instance) = create_instance();
    let surface = Surface::new(instance, window).unwrap();
    (adapter, surface)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlendMode {
    Add,
    Alpha,
    ColorBurn,
    ColorDodge,
    Color,
    Darken,
    DarkenColor,
    Difference,
    Divide,
    Exclusion,
    HardLight,
    HardMix,
    LinearBurn,
    Multiply,
    Normal,
    Overlay,
    Screen,
    Subtract,
    VividLight,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Helpful GPU cache; only required if multiple renders happen per frame and they have very different contents.
///
/// Remark: If you drop this the game will stutter, so it is best to wait a few frames before dropping it.
#[derive(Default)]
pub struct Cache(PoolRef<Pool>);

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CalcVertexAttrsComputeMode {
    pub idx_ty: IndexType,
    pub skin: bool,
}

impl CalcVertexAttrsComputeMode {
    pub const U16: Self = Self {
        idx_ty: IndexType::U16,
        skin: false,
    };
    pub const U16_SKIN: Self = Self {
        idx_ty: IndexType::U16,
        skin: true,
    };
    pub const U32: Self = Self {
        idx_ty: IndexType::U32,
        skin: false,
    };
    pub const U32_SKIN: Self = Self {
        idx_ty: IndexType::U32,
        skin: true,
    };
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct ColorRenderPassMode {
    fmt: Format,
    preserve: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum ComputeMode {
    CalcVertexAttrs(CalcVertexAttrsComputeMode),
    DecodeRgbRgba,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct DrawRenderPassMode {
    depth: Format,
    geom_buf: Format,
    light: Format,
    output: Format,
}

/// Allows you to load resources and begin rendering operations.
pub struct Gpu {
    driver: Driver,
    loads: LoadCache,
    ops: OpCache,
    renders: Cache,
}

impl Gpu {
    pub(super) fn new(window: &Window) -> (Self, Driver, Surface) {
        let (adapter, surface) = create_surface(window);

        info!(
            "Device: {} ({:?})",
            &adapter.info.name, adapter.info.device_type
        );

        let queue = adapter
            .queue_families
            .iter()
            .find(|family| {
                let ty = family.queue_type();
                surface.supports_queue_family(family)
                    && ty.supports_graphics()
                    && ty.supports_compute()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();
        let driver = Driver::new(RefCell::new(
            Device::new(adapter.physical_device, queue).unwrap(),
        ));
        let driver_copy = Driver::clone(&driver);
        (
            Self {
                driver,
                loads: Default::default(),
                ops: Default::default(),
                renders: Default::default(),
            },
            driver_copy,
            surface,
        )
    }

    // TODO: This is a useful function, but things you end up rendering with it cannot be used with the window's presentation
    // surface. Maybe change the way this whole thing works. Or document better?
    pub fn offscreen() -> Self {
        let (adapter, _) = create_instance();
        let queue = adapter
            .queue_families
            .iter()
            .find(|family| {
                let ty = family.queue_type();
                ty.supports_graphics() && ty.supports_compute()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();
        let driver = Driver::new(RefCell::new(
            Device::new(adapter.physical_device, queue).unwrap(),
        ));

        Self {
            driver,
            loads: Default::default(),
            ops: Default::default(),
            renders: Default::default(),
        }
    }

    pub fn load_animation<R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] _name: &str,
        _pak: &mut Pak<R>,
        _id: AnimationId,
    ) -> ModelRef {
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

    pub fn load_bitmap<R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: BitmapId,
    ) -> Bitmap {
        let bitmap = pak.read_bitmap(id);
        let mut pool = self.loads.borrow_mut();

        unsafe {
            BitmapOp::new(
                #[cfg(feature = "debug-names")]
                name,
                &self.driver,
                &mut pool,
                &bitmap,
            )
            .record()
        }
    }

    /// Only bitmapped fonts are supported.
    pub fn load_font<F: AsRef<str>, R: Read + Seek>(&self, pak: &mut Pak<R>, face: F) -> Font {
        #[cfg(debug_assertions)]
        debug!("Loading font `{}`", face.as_ref());

        Font::load(
            &self.driver,
            &mut self.loads.borrow_mut(),
            pak,
            face.as_ref(),
        )
    }

    pub fn load_model<R: Read + Seek>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        pak: &mut Pak<R>,
        id: ModelId,
    ) -> Model {
        let mut pool = self.loads.borrow_mut();
        let model = pak.read_model(id);

        // Create an index buffer
        let (idx_buf, idx_buf_len) = {
            let src = model.indices();
            let len = src.len() as _;
            let mut buf = pool.data_usage(
                #[cfg(feature = "debug-names")]
                name,
                &self.driver,
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
                &self.driver,
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
                &self.driver,
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
            &self.driver,
            vertex_buf_len,
            Usage::STORAGE | Usage::VERTEX,
        );

        Model::new(
            meshes,
            idx_ty,
            (idx_buf, idx_buf_len),
            (vertex_buf, vertex_buf_len),
            (staging_buf, staging_buf_len, write_mask),
        )
    }

    pub fn render(&self, #[cfg(feature = "debug-names")] name: &str, dims: Extent) -> Render {
        self.render_with_cache(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            &self.renders,
        )
    }

    pub fn render_with_cache(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        dims: Extent,
        cache: &Cache,
    ) -> Render {
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
        let driver = Driver::clone(&self.driver);

        Render::new(
            #[cfg(feature = "debug-names")]
            name,
            driver,
            dims,
            pool,
            ops,
        )
    }

    /// Resolves a render into a texture which can be written to other renders.
    pub fn resolve(&self, render: Render) -> Lease<Texture2d> {
        let (target, ops) = render.resolve();
        let mut cache = self.ops.borrow_mut();
        if let Some(cache) = cache.as_mut() {
            cache.extend(ops);
        } else {
            cache.replace(ops);
        }

        target
    }

    pub(crate) fn wait_idle(&self) {
        #[cfg(debug_assertions)]
        let started = Instant::now();

        // We are required to wait for the GPU to finish what we submitted before dropping the driver
        self.driver.borrow().wait_idle().unwrap();

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

impl Drop for Gpu {
    fn drop(&mut self) {
        self.wait_idle();
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum GraphicsMode {
    Blend(BlendMode),
    Font,
    FontOutline,
    Gradient,
    GradientTransparency,
    DrawLine,
    DrawMesh,
    DrawPointLight,
    DrawRectLight,
    DrawSpotlight,
    DrawSunlight,
    Texture,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum RenderPassMode {
    Color(ColorRenderPassMode),
    Draw(DrawRenderPassMode),
}
