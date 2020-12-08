pub mod spirv {
    include!(concat!(env!("OUT_DIR"), "/spirv/mod.rs"));
}

mod compute;
mod graphics;
mod lease;
mod render_passes;

pub use self::{
    compute::Compute,
    graphics::{FontVertex, Graphics},
    lease::Lease,
    render_passes::{color, draw, present},
};

use {
    super::{
        driver::{CommandPool, DescriptorPool, Driver, Fence, Image2d, Memory, RenderPass},
        op::Compiler,
        BlendMode, Data, Texture, TextureRef,
    },
    crate::math::Extent,
    gfx_hal::{
        buffer::Usage as BufferUsage,
        format::Format,
        image::{Layout, Tiling, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{DescriptorRangeDesc, DescriptorType},
        queue::QueueFamilyId,
        MemoryTypeId,
    },
    std::{
        cell::RefCell,
        collections::{HashMap, VecDeque},
        rc::Rc,
    },
};

#[cfg(debug_assertions)]
use gfx_hal::device::Device as _;

fn remove_last_by<T, F: Fn(&T) -> bool>(items: &mut VecDeque<T>, f: F) -> Option<T> {
    // let len = items.len();
    // TODO: This is no longer remove by last!!
    for idx in 0..items.len() {
        if f(&items[idx]) {
            return Some(items.remove(idx).unwrap());
        }
    }

    None
}

pub(super) type PoolRef<T> = Rc<RefCell<VecDeque<T>>>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ColorRenderPassMode {
    pub format: Format,
    pub preserve: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum ComputeMode {
    DecodeRgbRgba,
}

#[derive(Eq, Hash, PartialEq)]
struct DescriptorPoolKey {
    desc_ranges: Vec<(DescriptorType, usize)>,
}

pub struct Drain<'a>(&'a mut Pool);

impl<'a> Iterator for Drain<'a> {
    type Item = ();

    fn next(&mut self) -> Option<()> {
        unimplemented!();
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct DrawRenderPassMode {
    pub albedo: Format,
    pub depth: Format,

    /// Single channel accumulator
    pub light: Format,

    /// Dual channel (metal + roughness)
    pub material: Format,

    pub normal: Format,
}

#[derive(Eq, Hash, PartialEq)]
struct GraphicsKey {
    graphics_mode: GraphicsMode,
    render_pass_mode: RenderPassMode,
    subpass_idx: u8,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum GraphicsMode {
    Blend(BlendMode),
    Font,
    FontOutline,
    Gradient,
    GradientTransparency,
    DrawLine,
    DrawMesh,
    DrawMeshAnimated,
    Texture,
}

#[derive(Default)]
pub struct Pool {
    cmd_pools: HashMap<QueueFamilyId, PoolRef<CommandPool>>,
    compilers: PoolRef<Compiler>,
    computes: HashMap<ComputeMode, PoolRef<Compute>>,
    data: HashMap<BufferUsage, PoolRef<Data>>,
    desc_pools: HashMap<DescriptorPoolKey, PoolRef<DescriptorPool>>,
    fences: PoolRef<Fence>,
    graphics: HashMap<GraphicsKey, PoolRef<Graphics>>,
    memories: HashMap<MemoryTypeId, PoolRef<Memory>>,
    render_passes: HashMap<RenderPassMode, RenderPass>,
    textures: HashMap<TextureKey, PoolRef<TextureRef<Image2d>>>,
}

impl Pool {
    pub(crate) fn cmd_pool(
        &mut self,
        driver: &Driver,
        family: QueueFamilyId,
    ) -> Lease<CommandPool> {
        let items = self
            .cmd_pools
            .entry(family)
            .or_insert_with(Default::default);
        let mut item = if let Some(item) = items.borrow_mut().pop_back() {
            item
        } else {
            CommandPool::new(Driver::clone(driver), family)
        };

        unsafe {
            item.as_mut().reset(false);
        }

        Lease::new(item, items)
    }

    pub(crate) fn compiler(&mut self) -> Lease<Compiler> {
        let item = if let Some(item) = self.compilers.borrow_mut().pop_back() {
            item
        } else {
            debug!("Creating new compiler");
            Default::default()
        };

        Lease::new(item, &self.compilers)
    }

    pub(crate) fn compute(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        mode: ComputeMode,
    ) -> Lease<Compute> {
        let items = self.computes.entry(mode).or_insert_with(Default::default);
        let item = if let Some(item) = items.borrow_mut().pop_back() {
            item
        } else {
            let ctor = match mode {
                ComputeMode::DecodeRgbRgba => Compute::decode_rgb_rgba,
            };
            ctor(
                #[cfg(debug_assertions)]
                name,
                driver,
            )
        };

        Lease::new(item, items)
    }

    pub(crate) fn data(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        len: u64,
    ) -> Lease<Data> {
        self.data_usage(
            #[cfg(debug_assertions)]
            name,
            driver,
            len,
            BufferUsage::empty(),
        )
    }

    pub(crate) fn data_usage(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        len: u64,
        usage: BufferUsage,
    ) -> Lease<Data> {
        let items = self.data.entry(usage).or_insert_with(Default::default);
        let item = if let Some(item) =
            remove_last_by(&mut items.borrow_mut(), |item| item.capacity() >= len)
        {
            item
        } else {
            Data::new(
                #[cfg(debug_assertions)]
                name,
                Driver::clone(driver),
                len,
                usage,
            )
        };

        Lease::new(item, items)
    }

    // TODO: I don't really like the function signature here
    pub(crate) fn desc_pool<'i, I>(
        &mut self,
        driver: &Driver,
        max_sets: usize,
        desc_ranges: I,
    ) -> Lease<DescriptorPool>
    where
        I: Clone + ExactSizeIterator<Item = &'i DescriptorRangeDesc>,
    {
        let desc_ranges_key = desc_ranges
            .clone()
            .map(|desc_range| (desc_range.ty, desc_range.count))
            .collect();
        // TODO: Sort (and possibly combine) desc_ranges so that different orders of the same data don't affect key lookups
        let items = self
            .desc_pools
            .entry(DescriptorPoolKey {
                desc_ranges: desc_ranges_key,
            })
            .or_insert_with(Default::default);
        let item = if let Some(item) = remove_last_by(&mut items.borrow_mut(), |item| {
            DescriptorPool::max_sets(&item) >= max_sets
        }) {
            item
        } else {
            DescriptorPool::new(Driver::clone(driver), max_sets, desc_ranges)
        };

        Lease::new(item, items)
    }

    /// Allows callers to remove unused memory-consuming items from the pool.
    pub fn drain(&mut self) -> Drain {
        Drain(self)
    }

    pub(crate) fn fence(&mut self, driver: &Driver) -> Lease<Fence> {
        let item = if let Some(mut item) = self.fences.borrow_mut().pop_back() {
            Fence::reset(&mut item);
            item
        } else {
            Fence::new(Driver::clone(driver))
        };

        Lease::new(item, &self.fences)
    }

    pub(crate) fn graphics(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        graphics_mode: GraphicsMode,
        render_pass_mode: RenderPassMode,
        subpass_idx: u8,
    ) -> Lease<Graphics> {
        self.graphics_sets(
            #[cfg(debug_assertions)]
            name,
            driver,
            graphics_mode,
            render_pass_mode,
            subpass_idx,
            1,
        )
    }

    pub(crate) fn graphics_sets(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        graphics_mode: GraphicsMode,
        render_pass_mode: RenderPassMode,
        subpass_idx: u8,
        max_sets: usize,
    ) -> Lease<Graphics> {
        {
            let items = self
                .graphics
                .entry(GraphicsKey {
                    graphics_mode,
                    render_pass_mode,
                    subpass_idx,
                })
                .or_insert_with(Default::default);
            if let Some(item) =
                remove_last_by(&mut items.borrow_mut(), |item| item.max_sets() >= max_sets)
            {
                return Lease::new(item, items);
            }
        }
        let ctor = match graphics_mode {
            GraphicsMode::Blend(BlendMode::Normal) => Graphics::blend_normal,
            GraphicsMode::DrawLine => Graphics::draw_line,
            GraphicsMode::DrawMesh => Graphics::draw_mesh,
            GraphicsMode::Font => Graphics::font,
            GraphicsMode::FontOutline => Graphics::font_outline,
            GraphicsMode::Gradient => Graphics::gradient,
            GraphicsMode::GradientTransparency => Graphics::gradient_transparency,
            GraphicsMode::Texture => Graphics::texture,
            _ => panic!(),
        };
        let item = unsafe {
            ctor(
                #[cfg(debug_assertions)]
                name,
                driver,
                RenderPass::subpass(self.render_pass(driver, render_pass_mode), subpass_idx),
                max_sets,
            )
        };

        let items = &self.graphics[&GraphicsKey {
            graphics_mode,
            render_pass_mode,
            subpass_idx,
        }];
        Lease::new(item, items)
    }

    pub(crate) fn memory(
        &mut self,
        driver: &Driver,
        mem_type: MemoryTypeId,
        size: u64,
    ) -> Lease<Memory> {
        let items = self
            .memories
            .entry(mem_type)
            .or_insert_with(Default::default);
        let item = if let Some(item) =
            remove_last_by(&mut items.borrow_mut(), |item| Memory::size(&item) >= size)
        {
            item
        } else {
            Memory::new(Driver::clone(driver), mem_type, size)
        };

        Lease::new(item, items)
    }

    pub(crate) fn render_pass(&mut self, driver: &Driver, mode: RenderPassMode) -> &RenderPass {
        let driver = Driver::clone(driver);
        self.render_passes
            .entry(mode)
            .or_insert_with(|| match mode {
                RenderPassMode::Color(mode) => color(driver, mode),
                RenderPassMode::Draw(mode) => draw(driver, mode),
            })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn texture(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        driver: &Driver,
        dims: Extent,
        desired_tiling: Tiling,
        desired_fmts: &[Format],
        layout: Layout,
        usage: ImageUsage,
        layers: u16,
        mips: u8,
        samples: u8,
    ) -> Lease<TextureRef<Image2d>> {
        assert!(!desired_fmts.is_empty());

        let items = self
            .textures
            .entry(TextureKey {
                dims,
                desired_fmt: desired_fmts[0],
                layers,
                mips,
                samples,
                usage,
            })
            .or_insert_with(Default::default);
        let item = {
            let mut items_ref = items.as_ref().borrow_mut();
            if let Some(item) = items_ref.pop_back() {
                // Set a new name on this texture
                #[cfg(debug_assertions)]
                unsafe {
                    driver
                        .as_ref()
                        .borrow()
                        .set_image_name(item.as_ref().borrow_mut().as_mut(), name);
                }

                item
            } else {
                // Add a cache item so there will be an unused item waiting next time
                items_ref.push_front(TextureRef::new(RefCell::new(Texture::new(
                    #[cfg(debug_assertions)]
                    &format!("{} (Unused)", name),
                    Driver::clone(driver),
                    dims,
                    desired_tiling,
                    desired_fmts,
                    layout,
                    usage,
                    layers,
                    samples,
                    mips,
                ))));

                // Return a brand new instance
                TextureRef::new(RefCell::new(Texture::new(
                    #[cfg(debug_assertions)]
                    name,
                    Driver::clone(driver),
                    dims,
                    desired_tiling,
                    desired_fmts,
                    layout,
                    usage,
                    layers,
                    samples,
                    mips,
                )))
            }
        };

        Lease::new(item, items)
    }
}

#[derive(Eq, Hash, PartialEq)]
struct TextureKey {
    dims: Extent,
    desired_fmt: Format,
    layers: u16,
    mips: u8,
    samples: u8,
    usage: ImageUsage, // TODO: Usage shouldn't be a hard filter like this
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum RenderPassMode {
    Color(ColorRenderPassMode),
    Draw(DrawRenderPassMode),
}
