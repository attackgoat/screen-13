pub mod spirv {
    include!(concat!(env!("OUT_DIR"), "/spirv/mod.rs"));
}

mod compute;
mod graphics;
mod lease;
mod render_passes;

pub use self::{
    compute::{Compute, ComputeMode},
    graphics::{FontVertex, Graphics},
    lease::Lease,
    render_passes::{draw, read_write, read_write_ms, write, write_ms},
};

use {
    super::{
        driver::{CommandPool, DescriptorPool, Driver, Fence, Image2d, Memory, RenderPass},
        op::draw::Compiler,
        BlendMode, Data, Texture, TextureRef,
    },
    crate::math::Extent,
    gfx_hal::{
        buffer::Usage as BufferUsage,
        format::Format,
        image::{Layout, Tiling, Usage as ImageUsage},
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

pub(self) type PoolRef<T> = Rc<RefCell<VecDeque<T>>>;

#[derive(Debug, Eq, Hash, PartialEq)]
struct DescriptorPoolKey {
    desc_ranges: Vec<(DescriptorType, usize)>,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct GraphicsKey {
    graphics_mode: GraphicsMode,
    render_pass_mode: RenderPassMode,
    subpass_idx: u8,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GraphicsMode {
    Blend(BlendMode),
    Font,
    FontOutline,
    Gradient,
    GradientTransparency,
    Line,
    Mesh(MeshType),
    Spotlight,
    Sunlight,
    Texture,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MeshType {
    Animated,
    DualTexture,
    SingleTexture,
    Transparent,
}

#[derive(Debug)]
pub struct Pool {
    cmd_pools: HashMap<QueueFamilyId, PoolRef<CommandPool>>,
    compilers: PoolRef<Compiler>,
    computes: HashMap<ComputeMode, PoolRef<Compute>>,
    data: HashMap<BufferUsage, PoolRef<Data>>,
    desc_pools: HashMap<DescriptorPoolKey, PoolRef<DescriptorPool>>,
    driver: Driver,
    fences: PoolRef<Fence>,
    format: Format,
    graphics: HashMap<GraphicsKey, PoolRef<Graphics>>,
    memories: HashMap<MemoryTypeId, PoolRef<Memory>>,
    render_passes: HashMap<RenderPassMode, RenderPass>,
    textures: HashMap<TextureKey, PoolRef<TextureRef<Image2d>>>,
}

impl Pool {
    pub fn new(driver: &Driver, format: Format) -> Self {
        Self {
            cmd_pools: Default::default(),
            compilers: Default::default(),
            computes: Default::default(),
            data: Default::default(),
            desc_pools: Default::default(),
            driver: Driver::clone(driver),
            fences: Default::default(),
            format,
            graphics: Default::default(),
            memories: Default::default(),
            render_passes: Default::default(),
            textures: Default::default(),
        }
    }

    pub fn clear_textures(&mut self) {
        self.textures.clear();
    }

    pub fn cmd_pool(&mut self, family: QueueFamilyId) -> Lease<CommandPool> {
        let items = self
            .cmd_pools
            .entry(family)
            .or_insert_with(Default::default);
        let item = if let Some(item) = items.borrow_mut().pop_back() {
            item
        } else {
            CommandPool::new(Driver::clone(&self.driver), family)
        };

        Lease::new(item, items)
    }

    pub fn compiler(&mut self) -> Lease<Compiler> {
        let item = if let Some(item) = self.compilers.borrow_mut().pop_back() {
            item
        } else {
            Default::default()
        };

        Lease::new(item, &self.compilers)
    }

    pub fn compute(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        mode: ComputeMode,
    ) -> Lease<Compute> {
        let items = self.computes.entry(mode).or_insert_with(Default::default);
        let item = if let Some(item) = items.borrow_mut().pop_back() {
            item
        } else {
            let ctor = match mode {
                ComputeMode::DecodeBgr24 => Compute::decode_bgr24,
                ComputeMode::DecodeBgra32 => Compute::decode_bgra32,
            };
            ctor(
                #[cfg(debug_assertions)]
                name,
                &self.driver,
            )
        };

        Lease::new(item, items)
    }

    pub fn data(&mut self, #[cfg(debug_assertions)] name: &str, len: u64) -> Lease<Data> {
        self.data_usage(
            #[cfg(debug_assertions)]
            name,
            len,
            BufferUsage::empty(),
        )
    }

    pub fn data_usage(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
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
                Driver::clone(&self.driver),
                len,
                usage,
            )
        };

        Lease::new(item, items)
    }

    // TODO: I don't really like the function signature here
    pub fn desc_pool<'i, I>(&mut self, max_sets: usize, desc_ranges: I) -> Lease<DescriptorPool>
    where
        I: Clone + Iterator<Item = &'i DescriptorRangeDesc>,
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
            DescriptorPool::new(Driver::clone(&self.driver), max_sets, desc_ranges)
        };

        Lease::new(item, items)
    }

    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    pub fn fence(&mut self) -> Lease<Fence> {
        let item = if let Some(mut item) = self.fences.borrow_mut().pop_back() {
            Fence::reset(&mut item);
            item
        } else {
            Fence::new(Driver::clone(&self.driver))
        };

        Lease::new(item, &self.fences)
    }

    pub fn graphics(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        graphics_mode: GraphicsMode,
        render_pass_mode: RenderPassMode,
        subpass_idx: u8,
    ) -> Lease<Graphics> {
        self.graphics_sets(
            #[cfg(debug_assertions)]
            name,
            graphics_mode,
            render_pass_mode,
            subpass_idx,
            1,
        )
    }

    pub fn graphics_sets(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
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
            GraphicsMode::Font => Graphics::font,
            GraphicsMode::FontOutline => Graphics::font_outline,
            GraphicsMode::Gradient => Graphics::gradient,
            GraphicsMode::GradientTransparency => Graphics::gradient_transparency,
            GraphicsMode::Mesh(MeshType::DualTexture) => Graphics::draw_mesh_dual,
            GraphicsMode::Mesh(MeshType::SingleTexture) => Graphics::draw_mesh_single,
            GraphicsMode::Mesh(MeshType::Transparent) => Graphics::draw_trans,
            GraphicsMode::Spotlight => Graphics::draw_spotlight,
            GraphicsMode::Sunlight => Graphics::draw_sunlight,
            GraphicsMode::Texture => Graphics::texture,
            _ => panic!(),
        };
        let driver = Driver::clone(&self.driver); // TODO: Yuck
        let item = unsafe {
            ctor(
                #[cfg(debug_assertions)]
                name,
                &driver,
                RenderPass::subpass(self.render_pass(render_pass_mode), subpass_idx),
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

    pub fn memory(&mut self, mem_type: MemoryTypeId, size: u64) -> Lease<Memory> {
        let items = self
            .memories
            .entry(mem_type)
            .or_insert_with(Default::default);
        let item = if let Some(item) =
            remove_last_by(&mut items.borrow_mut(), |item| Memory::size(&item) >= size)
        {
            item
        } else {
            Memory::new(Driver::clone(&self.driver), mem_type, size)
        };

        Lease::new(item, items)
    }

    pub fn render_pass(&mut self, mode: RenderPassMode) -> &RenderPass {
        let driver = &self.driver;
        let format = self.format;
        self.render_passes
            .entry(mode)
            .or_insert_with(|| match mode {
                RenderPassMode::Draw => draw(driver, format),
                RenderPassMode::ReadWrite => read_write(driver, format),
                RenderPassMode::ReadWriteMs => read_write_ms(driver, format),
                RenderPassMode::Write => write(driver, format),
                RenderPassMode::WriteMs => write_ms(driver, format),
            })
    }

    pub fn set_format(&mut self, format: Format) {
        #[cfg(debug_assertions)]
        debug!("Setting GPU pool format to {:?}", format);
        self.format = format;

        // TODO: This is bad cause it drops the in-use renderpasses
        self.render_passes.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texture(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        dims: Extent,
        desired_tiling: Tiling,
        desired_format: Format,
        layout: Layout,
        usage: ImageUsage,
        layers: u16,
        mips: u8,
        samples: u8,
    ) -> Lease<TextureRef<Image2d>> {
        let items = self
            .textures
            .entry(TextureKey {
                dims,
                desired_format,
                layers,
                mips,
                samples,
                usage,
            })
            .or_insert_with(Default::default);
        let item = if let Some(item) = items.as_ref().borrow_mut().pop_back() {
            // Set a new name on this texture
            #[cfg(debug_assertions)]
            unsafe {
                self.driver
                    .as_ref()
                    .borrow()
                    .set_image_name(item.as_ref().borrow_mut().as_mut(), name);
            }

            item
        } else {
            TextureRef::new(RefCell::new(Texture::new(
                #[cfg(debug_assertions)]
                name,
                Driver::clone(&self.driver),
                dims,
                desired_tiling,
                desired_format,
                layout,
                usage,
                layers,
                samples,
                mips,
            )))
        };

        Lease::new(item, items)
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct TextureKey {
    dims: Extent,
    desired_format: Format,
    layers: u16,
    mips: u8,
    samples: u8,
    usage: ImageUsage, // TODO: Usage shouldn't be a hard filter like this
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RenderPassMode {
    Draw,
    ReadWrite,
    ReadWriteMs,
    Write,
    WriteMs,
}
