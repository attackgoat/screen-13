mod data;

/// A collection of smart-pointer types used internally to operate the GFX-HAL API.
mod driver;

/// A collection of operation implementations used internally to fulfill the Render API.
mod op;

/// A collection of resource pool types used internally to cache GFX-HAL types.
mod pool;

mod render;
mod texture;

pub use self::{
    op::{
        draw::{Command, Material},
        Bitmap, Font, Write, WriteMode,
    },
    render::{Operation, Render},
    texture::Texture,
};

pub(crate) use self::{
    driver::{Driver, PhysicalDevice, Swapchain},
    texture::Image,
};

use {
    self::{
        data::{Data, Mapping},
        driver::{open, Image2d, Surface},
        op::BitmapOp,
        pool::{Lease, Pool},
    },
    crate::{
        math::{Extent, Sphere},
        pak::{BitmapId, Pak},
        Error,
    },
    gfx_hal::{
        adapter::Adapter, buffer::Usage, device::Device, format::Format, queue::QueueFamily,
        window::Surface as _, Instance,
    },
    gfx_impl::{Backend as _Backend, Instance as InstanceImpl},
    std::{
        cell::RefCell,
        collections::HashMap,
        fmt::{Debug, Error as FmtError, Formatter},
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

// TODO: Make configurable!
pub const MULTISAMPLE_COUNT: u8 = 4;
// const DIRECTIONAL_SHADOW_BUFFERS: usize = 1;
// const SPOT_SHADOW_BUFFERS: usize = 8;

/// A two dimensional rendering result.
pub type Texture2d = TextureRef<Image2d>;

pub(self) type BitmapRef = Rc<Bitmap>;
pub(self) type PoolRef = Rc<RefCell<Pool>>;
pub(crate) type TextureRef<I> = Rc<RefCell<Texture<I>>>;

type BitmapCache = RefCell<HashMap<BitmapId, BitmapRef>>;

fn create_adapter() -> (Adapter<_Backend>, InstanceImpl) {
    let instance = InstanceImpl::create("attackgoat/screen-13", 1).unwrap();
    let mut adapters = instance.enumerate_adapters();
    if adapters.is_empty() {
        // TODO: Error::adapter
    }
    let adapter = adapters.remove(0);
    (adapter, instance)
}

// TODO: Different path for webgl and need this -> #[cfg(any(feature = "vulkan", feature = "metal"))]
fn create_adapter_surface(window: &Window) -> (Adapter<_Backend>, Surface) {
    let (adapter, instance) = create_adapter();
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

/// Allows you to load resources and begin rendering operations.
pub struct Gpu {
    bitmaps: BitmapCache,
    driver: Driver,
    pool: PoolRef,
}

impl Gpu {
    pub(super) fn new(window: &Window) -> (Self, Surface) {
        let (adapter, surface) = create_adapter_surface(window);

        // Note to future self: These two sections of code are basically the same, however attempting
        // to fold them together produced something with multiple lifetimes and generic parameters
        // that had to be spelled out. It was horrific. Duplicating code seemed like a better choice.
        let compute_queue_family = adapter
            .queue_families
            .iter()
            .find(|queue_family| {
                let queue_type = queue_family.queue_type();
                surface.supports_queue_family(queue_family) && queue_type.supports_compute()
            })
            .ok_or_else(Error::compute_queue_family)
            .unwrap();
        let graphics_queue_family = adapter
            .queue_families
            .iter()
            .find(|queue_family| {
                let queue_type = queue_family.queue_type();
                surface.supports_queue_family(queue_family) && queue_type.supports_graphics()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();

        let mut queue_families = vec![graphics_queue_family];
        if compute_queue_family.id() != queue_families[0].id() {
            queue_families.push(compute_queue_family);
        }

        let driver = open(adapter.physical_device, queue_families.into_iter());
        let pool = PoolRef::new(RefCell::new(Pool::new(&driver, Format::Rgba8Unorm)));

        (
            Self {
                bitmaps: Default::default(),
                driver,
                pool,
            },
            surface,
        )
    }

    pub fn offscreen() -> Self {
        let (adapter, _) = create_adapter();

        let compute_queue_family = adapter
            .queue_families
            .iter()
            .find(|queue_family| {
                let queue_type = queue_family.queue_type();
                queue_type.supports_compute()
            })
            .ok_or_else(Error::compute_queue_family)
            .unwrap();
        let graphics_queue_family = adapter
            .queue_families
            .iter()
            .find(|queue_family| {
                let queue_type = queue_family.queue_type();
                queue_type.supports_graphics()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();

        let mut queue_families = vec![graphics_queue_family];
        if compute_queue_family.id() != queue_families[0].id() {
            queue_families.push(compute_queue_family);
        }

        let driver = open(adapter.physical_device, queue_families.into_iter());
        let pool = PoolRef::new(RefCell::new(Pool::new(&driver, Format::Rgba8Unorm)));

        Self {
            bitmaps: Default::default(),
            driver,
            pool,
        }
    }

    pub(crate) fn driver(&self) -> &Driver {
        &self.driver
    }

    pub fn load_bitmap<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Bitmap {
        #[cfg(debug_assertions)]
        debug!("Loading bitmap `{}`", key.as_ref());

        let bitmap = pak.read_bitmap(key.as_ref());
        let pool = PoolRef::clone(&self.pool);
        unsafe {
            BitmapOp::new(
                #[cfg(debug_assertions)]
                name,
                &pool,
                &bitmap,
                Format::Rgba8Unorm,
            )
            .record()
        }
    }

    /// Note: The specfied font face must exist in the `fonts` directory and have an `fnt` extension.
    /// Only bitmapped fonts are supported. TODO: Maybe file locations should not be forced like this?
    pub fn load_font<F: AsRef<str>, R: Read + Seek>(&self, pak: &mut Pak<R>, face: F) -> Font {
        #[cfg(debug_assertions)]
        debug!("Loading font `{}`", face.as_ref());

        let pool = PoolRef::clone(&self.pool);
        Font::load(
            &pool,
            pak,
            &format!("fonts/{}.fnt", face.as_ref()),
            Format::Rgba8Unorm,
        )
    }

    pub fn load_mesh<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> Mesh {
        #[cfg(debug_assertions)]
        debug!("Loading mesh `{}`", key.as_ref());

        let pool = PoolRef::clone(&self.pool);
        let mesh = pak.read_mesh(key.as_ref());
        let mut cache = self.bitmaps.borrow_mut();
        let mut has_alpha = false;
        let bitmaps = mesh
            .bitmaps()
            .iter()
            .map(|id| {
                let id = *id;
                let bitmap = pak.read_bitmap_id(id);
                has_alpha |= bitmap.has_alpha();
                (
                    id,
                    BitmapRef::clone(cache.entry(id).or_insert_with(|| {
                        #[cfg(debug_assertions)]
                        info!("Caching bitmap #{}", id.0);

                        BitmapRef::new(unsafe {
                            BitmapOp::new(
                                #[cfg(debug_assertions)]
                                name,
                                &pool,
                                &bitmap,
                                Format::Rgba8Unorm,
                            )
                            .record()
                        })
                    })),
                )
            })
            .collect::<Vec<_>>();
        let vertices = mesh.vertices();
        let vertex_buf_len = vertices.len() as _;
        let mut vertex_buf = pool.borrow_mut().data_usage(
            #[cfg(debug_assertions)]
            name,
            vertex_buf_len,
            Usage::VERTEX,
        );

        {
            let mut mapped_range = vertex_buf.map_range_mut(0..vertex_buf_len).unwrap(); // TODO: Error handling!
            mapped_range.copy_from_slice(&vertices);
            Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
        }

        Mesh {
            bitmaps,
            bounds: mesh.bounds(),
            has_alpha,
            vertex_buf,
            vertex_count: vertices.len() as _,
        }
    }

    // TODO: This should not be exposed, bring users into this code?
    pub(crate) fn pool(&self) -> &PoolRef {
        &self.pool
    }

    pub fn render(&self, #[cfg(debug_assertions)] name: &str, dims: Extent) -> Render {
        Render::new(
            #[cfg(debug_assertions)]
            name,
            &self.pool,
            dims,
            Format::Rgba8Unorm,
        )
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

/// A textured and renderable model.
pub struct Mesh {
    bitmaps: Vec<(BitmapId, BitmapRef)>,
    bounds: Sphere,
    has_alpha: bool,
    vertex_buf: Lease<Data>,
    vertex_count: u32,
}

// TODO: Not sure about *anything* in this impl block. Maybe `textures`, that one is pretty cool.
impl Mesh {
    pub fn bounds(&self) -> Sphere {
        self.bounds
    }

    pub(crate) fn is_animated(&self) -> bool {
        // TODO: This needs to be implemented in some fashion - skys the limit here what should we do? hmmmm
        false
    }

    pub(crate) fn is_single_texture(&self) -> bool {
        self.bitmaps.len() == 1
    }

    pub(crate) fn textures(&self) -> impl Iterator<Item = &Texture2d> {
        Textures {
            bitmaps: &self.bitmaps,
            idx: 0,
        }
    }
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        f.write_str("Mesh")
    }
}

struct Textures<'a> {
    bitmaps: &'a Vec<(BitmapId, BitmapRef)>,
    idx: usize,
}

impl<'a> Iterator for Textures<'a> {
    type Item = &'a Texture2d;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.bitmaps.len() {
            Some(&self.bitmaps[self.idx].1)
        } else {
            None
        }
    }
}
