//! A collection of resource pool types used internally to cache GFX-HAL types.

mod layouts;
mod lease;

mod skydome {
    include!(concat!(env!("OUT_DIR"), "/skydome.rs"));
}

pub use self::lease::Lease;

use {
    self::{layouts::Layouts, skydome::SKYDOME},
    super::{
        def::{
            render_pass, CalcVertexAttrsComputeMode, Compute, ComputeMode, Graphics, GraphicsMode,
            RenderPassMode,
        },
        driver::{CommandPool, DescriptorPool, Fence, Image2d, Memory, RenderPass},
        op::draw::Compiler,
        physical_device, queue_family, BlendMode, Data, MaskMode, MatteMode, Texture, Texture2d,
        TextureRef,
    },
    crate::{math::Extent, Shared},
    archery::SharedPointerKind,
    gfx_hal::{
        adapter::PhysicalDevice as _,
        buffer::Usage as BufferUsage,
        format::{Format, ImageFeature, Properties},
        image::{Layout, Usage as ImageUsage},
        pool::CommandPool as _,
        pso::{DescriptorRangeDesc, DescriptorType},
        queue::QueueFamilyId,
        MemoryTypeId,
    },
    std::{
        cell::RefCell,
        collections::{HashMap, VecDeque},
    },
};

#[cfg(feature = "debug-names")]
use {super::device, gfx_hal::device::Device as _};

const DEFAULT_LRU_THRESHOLD: usize = 8;

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

pub(super) type PoolRef<T, P> = Shared<RefCell<VecDeque<T>>, P>;

#[derive(Eq, Hash, PartialEq)]
struct DescriptorPoolKey {
    desc_ranges: Vec<(DescriptorType, usize)>,
}

pub struct Drain<'a, P>(&'a mut Pool<P>)
where
    P: 'static + SharedPointerKind;

impl<'a, P> Iterator for Drain<'a, P>
where
    P: SharedPointerKind,
{
    type Item = ();

    fn next(&mut self) -> Option<()> {
        unimplemented!();
    }
}

#[derive(Eq, Hash, PartialEq)]
struct FormatKey {
    desired_fmt: Format,
    features: ImageFeature,
}

#[derive(Eq, Hash, PartialEq)]
struct GraphicsKey {
    graphics_mode: GraphicsMode,
    render_pass_mode: RenderPassMode,
    subpass_idx: u8,
}

pub struct Pool<P>
where
    P: 'static + SharedPointerKind,
{
    best_fmts: HashMap<FormatKey, Option<Format>>,
    cmd_pools: HashMap<QueueFamilyId, PoolRef<CommandPool, P>>,
    compilers: PoolRef<Compiler<P>, P>,
    computes: HashMap<ComputeMode, PoolRef<Compute, P>>,
    data: HashMap<BufferUsage, PoolRef<Data, P>>,
    desc_pools: HashMap<DescriptorPoolKey, PoolRef<DescriptorPool, P>>,
    fences: PoolRef<Fence, P>,
    graphics: HashMap<GraphicsKey, PoolRef<Graphics, P>>,
    pub(super) layouts: Layouts,

    /// The number of frames which must elapse before a least-recently-used cache item is considered obsolete.
    ///
    /// Remarks: Higher numbers such as 10 will use more memory but have less thrashing than lower numbers, such as 1.
    pub lru_threshold: usize,

    memories: HashMap<MemoryTypeId, PoolRef<Memory, P>>,
    render_passes: HashMap<RenderPassMode, RenderPass>,
    skydomes: PoolRef<Data, P>,
    textures: HashMap<TextureKey, PoolRef<Texture2d, P>>,
}

// TODO: Add some way to track memory usage so that using drain has some sort of feedback for users, tell them about the usage
impl<P> Pool<P>
where
    P: SharedPointerKind,
{
    /// Remarks: Only considers optimal tiling images.
    pub unsafe fn best_fmt(
        &mut self,
        desired_fmts: &[Format],
        features: ImageFeature,
    ) -> Option<Format> {
        assert!(!desired_fmts.is_empty());

        *self
            .best_fmts
            .entry(FormatKey {
                desired_fmt: desired_fmts[0],
                features,
            })
            .or_insert_with(|| {
                fn is_compatible(props: Properties, desired_features: ImageFeature) -> bool {
                    props.optimal_tiling.contains(desired_features)
                }

                for fmt in desired_fmts.iter() {
                    let props = physical_device().format_properties(Some(*fmt));
                    if is_compatible(props, features) {
                        // #[cfg(debug_assertions)]
                        // trace!(
                        //     "Picking format {:?} (desired {:?}) found (tiling={:?} usage={:?})",
                        //     *fmt, desired_fmts[0], tiling, usage
                        // );

                        return Some(*fmt);
                    }
                }

                #[cfg(debug_assertions)]
                {
                    let all_fmts = &[
                        Format::Rg4Unorm,
                        Format::Rgba4Unorm,
                        Format::Bgra4Unorm,
                        Format::R5g6b5Unorm,
                        Format::B5g6r5Unorm,
                        Format::R5g5b5a1Unorm,
                        Format::B5g5r5a1Unorm,
                        Format::A1r5g5b5Unorm,
                        Format::R8Unorm,
                        Format::R8Snorm,
                        Format::R8Uscaled,
                        Format::R8Sscaled,
                        Format::R8Uint,
                        Format::R8Sint,
                        Format::R8Srgb,
                        Format::Rg8Unorm,
                        Format::Rg8Snorm,
                        Format::Rg8Uscaled,
                        Format::Rg8Sscaled,
                        Format::Rg8Uint,
                        Format::Rg8Sint,
                        Format::Rg8Srgb,
                        Format::Rgb8Unorm,
                        Format::Rgb8Snorm,
                        Format::Rgb8Uscaled,
                        Format::Rgb8Sscaled,
                        Format::Rgb8Uint,
                        Format::Rgb8Sint,
                        Format::Rgb8Srgb,
                        Format::Bgr8Unorm,
                        Format::Bgr8Snorm,
                        Format::Bgr8Uscaled,
                        Format::Bgr8Sscaled,
                        Format::Bgr8Uint,
                        Format::Bgr8Sint,
                        Format::Bgr8Srgb,
                        Format::Rgba8Unorm,
                        Format::Rgba8Snorm,
                        Format::Rgba8Uscaled,
                        Format::Rgba8Sscaled,
                        Format::Rgba8Uint,
                        Format::Rgba8Sint,
                        Format::Rgba8Srgb,
                        Format::Bgra8Unorm,
                        Format::Bgra8Snorm,
                        Format::Bgra8Uscaled,
                        Format::Bgra8Sscaled,
                        Format::Bgra8Uint,
                        Format::Bgra8Sint,
                        Format::Bgra8Srgb,
                        Format::Abgr8Unorm,
                        Format::Abgr8Snorm,
                        Format::Abgr8Uscaled,
                        Format::Abgr8Sscaled,
                        Format::Abgr8Uint,
                        Format::Abgr8Sint,
                        Format::Abgr8Srgb,
                        Format::A2r10g10b10Unorm,
                        Format::A2r10g10b10Snorm,
                        Format::A2r10g10b10Uscaled,
                        Format::A2r10g10b10Sscaled,
                        Format::A2r10g10b10Uint,
                        Format::A2r10g10b10Sint,
                        Format::A2b10g10r10Unorm,
                        Format::A2b10g10r10Snorm,
                        Format::A2b10g10r10Uscaled,
                        Format::A2b10g10r10Sscaled,
                        Format::A2b10g10r10Uint,
                        Format::A2b10g10r10Sint,
                        Format::R16Unorm,
                        Format::R16Snorm,
                        Format::R16Uscaled,
                        Format::R16Sscaled,
                        Format::R16Uint,
                        Format::R16Sint,
                        Format::R16Sfloat,
                        Format::Rg16Unorm,
                        Format::Rg16Snorm,
                        Format::Rg16Uscaled,
                        Format::Rg16Sscaled,
                        Format::Rg16Uint,
                        Format::Rg16Sint,
                        Format::Rg16Sfloat,
                        Format::Rgb16Unorm,
                        Format::Rgb16Snorm,
                        Format::Rgb16Uscaled,
                        Format::Rgb16Sscaled,
                        Format::Rgb16Uint,
                        Format::Rgb16Sint,
                        Format::Rgb16Sfloat,
                        Format::Rgba16Unorm,
                        Format::Rgba16Snorm,
                        Format::Rgba16Uscaled,
                        Format::Rgba16Sscaled,
                        Format::Rgba16Uint,
                        Format::Rgba16Sint,
                        Format::Rgba16Sfloat,
                        Format::R32Uint,
                        Format::R32Sint,
                        Format::R32Sfloat,
                        Format::Rg32Uint,
                        Format::Rg32Sint,
                        Format::Rg32Sfloat,
                        Format::Rgb32Uint,
                        Format::Rgb32Sint,
                        Format::Rgb32Sfloat,
                        Format::Rgba32Uint,
                        Format::Rgba32Sint,
                        Format::Rgba32Sfloat,
                        Format::R64Uint,
                        Format::R64Sint,
                        Format::R64Sfloat,
                        Format::Rg64Uint,
                        Format::Rg64Sint,
                        Format::Rg64Sfloat,
                        Format::Rgb64Uint,
                        Format::Rgb64Sint,
                        Format::Rgb64Sfloat,
                        Format::Rgba64Uint,
                        Format::Rgba64Sint,
                        Format::Rgba64Sfloat,
                        Format::B10g11r11Ufloat,
                        Format::E5b9g9r9Ufloat,
                        Format::D16Unorm,
                        Format::X8D24Unorm,
                        Format::D32Sfloat,
                        Format::S8Uint,
                        Format::D16UnormS8Uint,
                        Format::D24UnormS8Uint,
                        Format::D32SfloatS8Uint,
                        Format::Bc1RgbUnorm,
                        Format::Bc1RgbSrgb,
                        Format::Bc1RgbaUnorm,
                        Format::Bc1RgbaSrgb,
                        Format::Bc2Unorm,
                        Format::Bc2Srgb,
                        Format::Bc3Unorm,
                        Format::Bc3Srgb,
                        Format::Bc4Unorm,
                        Format::Bc4Snorm,
                        Format::Bc5Unorm,
                        Format::Bc5Snorm,
                        Format::Bc6hUfloat,
                        Format::Bc6hSfloat,
                        Format::Bc7Unorm,
                        Format::Bc7Srgb,
                        Format::Etc2R8g8b8Unorm,
                        Format::Etc2R8g8b8Srgb,
                        Format::Etc2R8g8b8a1Unorm,
                        Format::Etc2R8g8b8a1Srgb,
                        Format::Etc2R8g8b8a8Unorm,
                        Format::Etc2R8g8b8a8Srgb,
                        Format::EacR11Unorm,
                        Format::EacR11Snorm,
                        Format::EacR11g11Unorm,
                        Format::EacR11g11Snorm,
                        Format::Astc4x4Unorm,
                        Format::Astc4x4Srgb,
                        Format::Astc5x4Unorm,
                        Format::Astc5x4Srgb,
                        Format::Astc5x5Unorm,
                        Format::Astc5x5Srgb,
                        Format::Astc6x5Unorm,
                        Format::Astc6x5Srgb,
                        Format::Astc6x6Unorm,
                        Format::Astc6x6Srgb,
                        Format::Astc8x5Unorm,
                        Format::Astc8x5Srgb,
                        Format::Astc8x6Unorm,
                        Format::Astc8x6Srgb,
                        Format::Astc8x8Unorm,
                        Format::Astc8x8Srgb,
                        Format::Astc10x5Unorm,
                        Format::Astc10x5Srgb,
                        Format::Astc10x6Unorm,
                        Format::Astc10x6Srgb,
                        Format::Astc10x8Unorm,
                        Format::Astc10x8Srgb,
                        Format::Astc10x10Unorm,
                        Format::Astc10x10Srgb,
                        Format::Astc12x10Unorm,
                        Format::Astc12x10Srgb,
                        Format::Astc12x12Unorm,
                        Format::Astc12x12Srgb,
                    ];

                    let mut compatible_fmts = vec![];
                    for fmt in all_fmts.iter() {
                        if is_compatible(physical_device().format_properties(Some(*fmt)), features)
                        {
                            compatible_fmts.push(*fmt);
                        }
                    }

                    warn!(
                        "A desired compatible format was not found for `{:?}` (Features={:?})",
                        desired_fmts[0], features
                    );

                    if !compatible_fmts.is_empty() {
                        info!(
                            "These formats are compatible: {}",
                            &compatible_fmts
                                .iter()
                                .map(|format| format!("{:?}", format))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }

                None
            })
    }

    pub(super) unsafe fn cmd_pool(&mut self) -> Lease<CommandPool, P> {
        self.cmd_pool_with_family(queue_family())
    }

    pub(super) unsafe fn cmd_pool_with_family(
        &mut self,
        family: QueueFamilyId,
    ) -> Lease<CommandPool, P> {
        // let items = self
        //     .cmd_pools
        //     .entry(family)
        //     .or_insert_with(Default::default);
        // let mut item = if let Some(item) = items.borrow_mut().pop_back() {
        //     item
        // } else {
        //     CommandPool::new(family)
        // };

        // item.as_mut().reset(false);

        // Lease::new(item, items)
        todo!("DONT CHECKIN");
    }

    pub(super) fn compiler(&mut self) -> Lease<Compiler<P>, P> {
        let item = if let Some(item) = self.compilers.borrow_mut().pop_back() {
            item
        } else {
            //Default::default()
            todo!("DONT CHECKIN");
        };

        Lease::new(item, &self.compilers)
    }

    /// Returns a lease to a compute pipeline with no descriptor sets.
    pub(super) unsafe fn compute(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        mode: ComputeMode,
    ) -> Lease<Compute, P> {
        self.compute_desc_sets(
            #[cfg(feature = "debug-names")]
            name,
            mode,
            0,
        )
    }

    /// Returns a lease to a compute pipeline with the specified number of descriptor sets.
    pub(super) unsafe fn compute_desc_sets(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        mode: ComputeMode,
        max_desc_sets: usize,
    ) -> Lease<Compute, P> {
        // let items = self.computes.entry(mode).or_insert_with(Default::default);
        // let item = if let Some(item) = remove_last_by(&mut items.borrow_mut(), |item| {
        //     item.max_desc_sets() >= max_desc_sets
        // }) {
        //     item
        // } else {
        //     let ctor = match mode {
        //         ComputeMode::CalcVertexAttrs(mode) => match mode {
        //             CalcVertexAttrsComputeMode::U16 => Compute::calc_vertex_attrs_u16,
        //             CalcVertexAttrsComputeMode::U16_SKIN => Compute::calc_vertex_attrs_u16_skin,
        //             CalcVertexAttrsComputeMode::U32 => Compute::calc_vertex_attrs_u32,
        //             CalcVertexAttrsComputeMode::U32_SKIN => Compute::calc_vertex_attrs_u32_skin,
        //         },
        //         ComputeMode::DecodeRgbRgba => Compute::decode_rgb_rgba,
        //     };
        //     let (desc_set_layout, pipeline_layout) = match mode {
        //         ComputeMode::CalcVertexAttrs(_) => self.layouts.compute_calc_vertex_attrs(
        //             #[cfg(feature = "debug-names")]
        //             name,
        //         ),
        //         ComputeMode::DecodeRgbRgba => self.layouts.compute_decode_rgb_rgba(
        //             #[cfg(feature = "debug-names")]
        //             name,
        //         ),
        //     };

        //     ctor(
        //         #[cfg(feature = "debug-names")]
        //         name,
        //         desc_set_layout,
        //         pipeline_layout,
        //         max_desc_sets,
        //     )
        // };

        // Lease::new(item, items)

        todo!("DONT CHECKIN");
    }

    pub(super) unsafe fn data(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        len: u64,
    ) -> Lease<Data, P> {
        self.data_usage(
            #[cfg(feature = "debug-names")]
            name,
            len,
            BufferUsage::empty(),
        )
    }

    pub(super) unsafe fn data_usage(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        len: u64,
        usage: BufferUsage,
    ) -> Lease<Data, P> {
        // let items = self.data.entry(usage).or_insert_with(Default::default);
        // let item = if let Some(item) =
        //     remove_last_by(&mut items.borrow_mut(), |item| item.capacity() >= len)
        // {
        //     item
        // } else {
        //     Data::new(
        //         #[cfg(feature = "debug-names")]
        //         name,
        //         len,
        //         usage,
        //     )
        // };

        // Lease::new(item, items)

        todo!("DONT CHECKIN");
    }

    pub(super) unsafe fn desc_pool<'i, I>(
        &mut self,
        max_desc_sets: usize,
        desc_ranges: I,
    ) -> Lease<DescriptorPool, P>
    where
        I: Clone + ExactSizeIterator<Item = &'i DescriptorRangeDesc>,
    {
        // let desc_ranges_key = desc_ranges
        //     .clone()
        //     .map(|desc_range| (desc_range.ty, desc_range.count))
        //     .collect();
        // // TODO: Sort (and possibly combine) desc_ranges so that different orders of the same data don't affect key lookups
        // let items = self
        //     .desc_pools
        //     .entry(DescriptorPoolKey {
        //         desc_ranges: desc_ranges_key,
        //     })
        //     .or_insert_with(Default::default);
        // let item = if let Some(item) = remove_last_by(&mut items.borrow_mut(), |item| {
        //     DescriptorPool::max_desc_sets(&item) >= max_desc_sets
        // }) {
        //     item
        // } else {
        //     DescriptorPool::new(max_desc_sets, desc_ranges)
        // };

        // Lease::new(item, items)
        todo!("DONT CHECKIN");
    }

    /// Allows callers to remove unused memory-consuming items from the pool.
    pub fn drain(&mut self) -> Drain<'_, P> {
        Drain(self)
    }

    pub(super) unsafe fn fence(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> Lease<Fence, P> {
        let item = if let Some(mut item) = self.fences.borrow_mut().pop_back() {
            Fence::reset(&mut item);
            item
        } else {
            Fence::new(
                #[cfg(feature = "debug-names")]
                name,
            )
        };

        Lease::new(item, &self.fences)
    }

    /// Returns a lease to a graphics pipeline with no descriptor sets.
    pub(super) unsafe fn graphics(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        render_pass_mode: RenderPassMode,
        subpass_idx: u8,
        graphics_mode: GraphicsMode,
    ) -> Lease<Graphics, P> {
        self.graphics_desc_sets(
            #[cfg(feature = "debug-names")]
            name,
            render_pass_mode,
            subpass_idx,
            graphics_mode,
            0,
        )
    }

    /// Returns a lease to a graphics pipeline with the specified number of descriptor sets.
    pub(super) unsafe fn graphics_desc_sets(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        render_pass_mode: RenderPassMode,
        subpass_idx: u8,
        graphics_mode: GraphicsMode,
        max_desc_sets: usize,
    ) -> Lease<Graphics, P> {
        // {
        //     let items = self
        //         .graphics
        //         .entry(GraphicsKey {
        //             graphics_mode,
        //             render_pass_mode,
        //             subpass_idx,
        //         })
        //         .or_insert_with(Default::default);
        //     if let Some(item) = remove_last_by(&mut items.borrow_mut(), |item| {
        //         item.max_desc_sets() >= max_desc_sets
        //     }) {
        //         return Lease::new(item, items);
        //     }
        // }
        // let ctor = match graphics_mode {
        //     GraphicsMode::Blend(BlendMode::Add) => Graphics::blend_add,
        //     GraphicsMode::Blend(BlendMode::AlphaAdd) => Graphics::blend_alpha_add,
        //     GraphicsMode::Blend(BlendMode::ColorBurn) => Graphics::blend_color_burn,
        //     GraphicsMode::Blend(BlendMode::ColorDodge) => Graphics::blend_color_dodge,
        //     GraphicsMode::Blend(BlendMode::Color) => Graphics::blend_color,
        //     GraphicsMode::Blend(BlendMode::Darken) => Graphics::blend_darken,
        //     GraphicsMode::Blend(BlendMode::DarkerColor) => Graphics::blend_darker_color,
        //     GraphicsMode::Blend(BlendMode::Difference) => Graphics::blend_difference,
        //     GraphicsMode::Blend(BlendMode::Divide) => Graphics::blend_divide,
        //     GraphicsMode::Blend(BlendMode::Exclusion) => Graphics::blend_exclusion,
        //     GraphicsMode::Blend(BlendMode::HardLight) => Graphics::blend_hard_light,
        //     GraphicsMode::Blend(BlendMode::HardMix) => Graphics::blend_hard_mix,
        //     GraphicsMode::Blend(BlendMode::LinearBurn) => Graphics::blend_linear_burn,
        //     GraphicsMode::Blend(BlendMode::Multiply) => Graphics::blend_multiply,
        //     GraphicsMode::Blend(BlendMode::Normal) => Graphics::blend_normal,
        //     GraphicsMode::Blend(BlendMode::Overlay) => Graphics::blend_overlay,
        //     GraphicsMode::Blend(BlendMode::Screen) => Graphics::blend_screen,
        //     GraphicsMode::Blend(BlendMode::Subtract) => Graphics::blend_subtract,
        //     GraphicsMode::Blend(BlendMode::VividLight) => Graphics::blend_vivid_light,
        //     GraphicsMode::DrawLine => Graphics::draw_line,
        //     GraphicsMode::DrawMesh => Graphics::draw_mesh,
        //     GraphicsMode::DrawPointLight => Graphics::draw_point_light,
        //     GraphicsMode::DrawRectLight => Graphics::draw_rect_light,
        //     GraphicsMode::DrawSpotlight => Graphics::draw_spotlight,
        //     GraphicsMode::DrawSunlight => Graphics::draw_sunlight,
        //     GraphicsMode::Font(false) => Graphics::font_normal,
        //     GraphicsMode::Font(true) => Graphics::font_outline,
        //     GraphicsMode::Gradient(false) => Graphics::gradient_linear,
        //     GraphicsMode::Gradient(true) => Graphics::gradient_linear_trans,
        //     GraphicsMode::Mask(MaskMode::Add) => Graphics::mask_add,
        //     GraphicsMode::Mask(MaskMode::Darken) => Graphics::mask_darken,
        //     GraphicsMode::Mask(MaskMode::Difference) => Graphics::mask_difference,
        //     GraphicsMode::Mask(MaskMode::Intersect) => Graphics::mask_intersect,
        //     GraphicsMode::Mask(MaskMode::Lighten) => Graphics::mask_lighten,
        //     GraphicsMode::Mask(MaskMode::Subtract) => Graphics::mask_subtract,
        //     GraphicsMode::Matte(MatteMode::Alpha) => Graphics::matte_alpha,
        //     GraphicsMode::Matte(MatteMode::AlphaInverted) => Graphics::matte_alpha_inv,
        //     GraphicsMode::Matte(MatteMode::Luminance) => Graphics::matte_luma,
        //     GraphicsMode::Matte(MatteMode::LuminanceInverted) => Graphics::matte_luma_inv,
        //     GraphicsMode::Skydome => Graphics::skydome,
        //     GraphicsMode::Texture => Graphics::texture,
        // };
        // let item = {
        //     let render_pass = self.render_pass(render_pass_mode);
        //     let subpass = RenderPass::subpass(render_pass, subpass_idx);
        //     ctor(
        //         #[cfg(feature = "debug-names")]
        //         name,
        //         subpass,
        //         max_desc_sets,
        //     )
        // };

        // let items = &self.graphics[&GraphicsKey {
        //     graphics_mode,
        //     render_pass_mode,
        //     subpass_idx,
        // }];
        // Lease::new(item, items)
        todo!("DONT CHECKIN");
    }

    pub(super) unsafe fn memory(&mut self, mem_type: MemoryTypeId, size: u64) -> Lease<Memory, P> {
        // let items = self
        //     .memories
        //     .entry(mem_type)
        //     .or_insert_with(Default::default);
        // let item = if let Some(item) =
        //     remove_last_by(&mut items.borrow_mut(), |item| Memory::size(&item) >= size)
        // {
        //     item
        // } else {
        //     Memory::new(mem_type, size)
        // };

        // Lease::new(item, items)

        todo!("DONT CHECKIN");
    }

    pub(super) unsafe fn render_pass(&mut self, mode: RenderPassMode) -> &RenderPass {
        self.render_passes
            .entry(mode)
            .or_insert_with(|| match mode {
                RenderPassMode::Color(mode) => render_pass::color(mode),
                RenderPassMode::Draw(mode) => {
                    if mode.skydome as u8 * mode.post_fx as u8 == 1 {
                        render_pass::draw::fill_skydome_light_tonemap_fx(mode)
                    } else if mode.skydome {
                        render_pass::draw::fill_skydome_light_tonemap(mode)
                    } else if mode.post_fx {
                        render_pass::draw::fill_light_tonemap_fx(mode)
                    } else {
                        render_pass::draw::fill_light_tonemap(mode)
                    }
                }
            })
    }

    /// This *highly* specialized pool function returns a fixed size Data which should be used
    /// only for skydome rendering. If the data is brand new then the skydome vertex data will
    /// be returned at the same time. It is up to the user to load it and provide the proper
    /// pipeline barriers. Good luck!
    pub(super) unsafe fn skydome(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
    ) -> (Lease<Data, P>, u64, Option<&[u8]>) {
        let (item, data) = if let Some(item) = self.skydomes.borrow_mut().pop_back() {
            (item, None)
        } else {
            let data = Data::new(
                #[cfg(feature = "debug-names")]
                name,
                SKYDOME.len() as _,
                BufferUsage::VERTEX,
            );

            (data, Some(SKYDOME.as_ref()))
        };

        (Lease::new(item, &self.skydomes), SKYDOME.len() as _, data)
    }

    // TODO: Bubble format picking up and out of this! (removes desire_tiling+desired_fmts+features, replace with fmt/tiling)
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn texture(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        dims: Extent,
        fmt: Format,
        layout: Layout,
        usage: ImageUsage,
        layers: u16,
        mips: u8,
        samples: u8,
    ) -> Lease<Texture2d, P> {
        // let items = self
        //     .textures
        //     .entry(TextureKey {
        //         dims,
        //         fmt,
        //         layers,
        //         mips,
        //         samples,
        //         usage,
        //     })
        //     .or_insert_with(Default::default);
        // let item = {
        //     let mut items_ref = items.as_ref().borrow_mut();
        //     if let Some(item) = items_ref.pop_back() {
        //         // Set a new name on this texture
        //         #[cfg(feature = "debug-names")]
        //         device().set_image_name(item.as_ref().borrow_mut().as_mut(), name);

        //         item
        //     } else {
        //         // Add a cache item so there will be an unused item waiting next time
        //         items_ref.push_front(TextureRef::new(RefCell::new(Texture::new(
        //             #[cfg(feature = "debug-names")]
        //             &format!("{} (Unused)", name),
        //             dims,
        //             fmt,
        //             layout,
        //             usage,
        //             layers,
        //             samples,
        //             mips,
        //         ))));

        //         // Return a brand new instance
        //         TextureRef::new(RefCell::new(Texture::new(
        //             #[cfg(feature = "debug-names")]
        //             name,
        //             dims,
        //             fmt,
        //             layout,
        //             usage,
        //             layers,
        //             samples,
        //             mips,
        //         )))
        //     }
        // };

        // Lease::new(item, items)

        todo!("DONT CHECKIN");
    }
}

impl<P> Default for Pool<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        // Self {
        //     best_fmts: Default::default(),
        //     cmd_pools: Default::default(),
        //     compilers: Default::default(),
        //     computes: Default::default(),
        //     data: Default::default(),
        //     desc_pools: Default::default(),
        //     fences: Default::default(),
        //     graphics: Default::default(),
        //     layouts: Default::default(),
        //     lru_threshold: DEFAULT_LRU_THRESHOLD,
        //     memories: Default::default(),
        //     render_passes: Default::default(),
        //     skydomes: Default::default(),
        //     textures: Default::default(),
        // }

        todo!("DONT CHECKIN");
    }
}

impl<P> Drop for Pool<P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        // Make sure these get dropped before the layouts! (They contain unsafe references!)
        self.computes.clear();
        self.graphics.clear();
    }
}

#[derive(Eq, Hash, PartialEq)]
struct TextureKey {
    dims: Extent,
    fmt: Format,
    layers: u16,
    mips: u8,
    samples: u8,
    usage: ImageUsage, // TODO: Usage shouldn't be a hard filter like this
}
