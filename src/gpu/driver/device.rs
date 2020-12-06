use {
    crate::{private::Sealed, Error},
    gfx_hal::{
        adapter::{MemoryProperties, PhysicalDevice as _},
        format::{Format, ImageFeature, Properties as FormatProperties},
        image::{Tiling, Usage},
        memory::Properties,
        queue::{QueueFamilyId, QueueGroup},
        Backend, Features, MemoryTypeId,
    },
    gfx_impl::Backend as _Backend,
    std::{
        cell::RefCell,
        collections::HashMap,
        ops::{Deref, DerefMut},
    },
};

// TODO: This only supports one queue family which creates one queue. We don't use async submissions yet but it would be super cool.
pub struct Device {
    fmts: RefCell<HashMap<FormatKey, Option<Format>>>,
    mem: MemoryProperties,
    phys: <_Backend as Backend>::PhysicalDevice,
    ptr: <_Backend as Backend>::Device,
    queue_group: QueueGroup<_Backend>,
}

impl Device {
    pub fn new(
        phys: <_Backend as Backend>::PhysicalDevice,
        queue: &<_Backend as Backend>::QueueFamily,
    ) -> Result<Self, Error> {
        let mem = phys.memory_properties();
        let mut gpu = unsafe { phys.open(&[(queue, &[1f32])], Features::empty())? };
        let queue_group = gpu.queue_groups.pop().unwrap();

        assert!(!queue_group.queues.is_empty());

        Ok(Self {
            fmts: Default::default(),
            mem,
            phys,
            ptr: gpu.device,
            queue_group,
        })
    }

    pub fn queue_family(device: &Self) -> QueueFamilyId {
        device.queue_group.family
    }
}

impl AsMut<<_Backend as Backend>::Device> for Device {
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Device {
        &mut *self
    }
}

impl AsRef<<_Backend as Backend>::Device> for Device {
    fn as_ref(&self) -> &<_Backend as Backend>::Device {
        &*self
    }
}

impl Deref for Device {
    type Target = <_Backend as Backend>::Device;

    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

impl DerefMut for Device {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptr
    }
}

impl PhysicalDevice for Device {
    fn best_fmt(&self, desired_fmts: &[Format], tiling: Tiling, usage: Usage) -> Option<Format> {
        assert!(!desired_fmts.is_empty());

        let mut fmts = self.fmts.borrow_mut();
        *fmts
            .entry(FormatKey {
                desired_fmt: desired_fmts[0],
                tiling,
                usage,
            })
            .or_insert_with(|| {
                fn is_supported(
                    usage: Usage,
                    usage_mask: Usage,
                    features: ImageFeature,
                    feature_mask: ImageFeature,
                ) -> bool {
                    let used = usage.contains(usage_mask);
                    let supported = features.contains(feature_mask);
                    supported || !used
                }

                fn is_compatible(props: FormatProperties, tiling: Tiling, usage: Usage) -> bool {
                    let features = match tiling {
                        Tiling::Linear => props.linear_tiling,
                        Tiling::Optimal => props.optimal_tiling,
                    };

                    is_supported(usage, Usage::SAMPLED, features, ImageFeature::SAMPLED)
                        && is_supported(usage, Usage::STORAGE, features, ImageFeature::STORAGE)
                        && is_supported(
                            usage,
                            Usage::COLOR_ATTACHMENT,
                            features,
                            ImageFeature::COLOR_ATTACHMENT,
                        )
                        && is_supported(
                            usage,
                            Usage::COLOR_ATTACHMENT,
                            features,
                            ImageFeature::COLOR_ATTACHMENT_BLEND,
                        )
                        && is_supported(
                            usage,
                            Usage::DEPTH_STENCIL_ATTACHMENT,
                            features,
                            ImageFeature::DEPTH_STENCIL_ATTACHMENT,
                        )
                        && is_supported(
                            usage,
                            Usage::TRANSFER_DST,
                            features,
                            ImageFeature::BLIT_DST,
                        )
                        && is_supported(
                            usage,
                            Usage::TRANSFER_SRC,
                            features,
                            ImageFeature::BLIT_SRC,
                        )
                }

                for fmt in desired_fmts.iter() {
                    if is_compatible(
                        self.phys.format_properties(Some(*fmt)),
                        tiling,
                        usage,
                    ) {
                        #[cfg(debug_assertions)]
                        debug!(
                            "Picking format {:?} (desired {:?}) found (tiling={:?} usage={:?})",
                            *fmt, desired_fmts[0], tiling, usage
                        );

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
                        if is_compatible(self.phys.format_properties(Some(*fmt)), tiling, usage) {
                            compatible_fmts.push(*fmt);
                        }
                    }

                    warn!(
                        "A desired compatible format was not found for `{:?}` (Tiling={:?} Usage={:?})",
                        desired_fmts[0], tiling, usage
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

    fn gpu(&self) -> &<_Backend as Backend>::PhysicalDevice {
        &self.phys
    }

    fn mem_ty(&self, mask: u32, properties: Properties) -> MemoryTypeId {
        //debug!("type_mask={} properties={:?}", type_mask, properties);
        self.mem
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, mem_ty)| {
                //debug!("Mem ID {} type={:?}", id, mem_type);
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                mask & (1 << id) != 0 && mem_ty.properties.contains(properties)
            })
            .unwrap()
            .into()
        /*
        for index in 0..self.mem.memory_types.len() {
            if type_mask & (1 << index) > 0 {
                let mem_type = &self.mem.memory_types[index];
                if properties == properties & mem_type.properties {
                    return MemoryTypeId(index);
                }
            }
        }

        panic!("Memory type not found");*/
    }

    fn queue_mut(&mut self) -> &mut <_Backend as Backend>::CommandQueue {
        &mut self.queue_group.queues[0]
    }
}

impl Sealed for Device {}

#[derive(Eq, Hash, PartialEq)]
struct FormatKey {
    desired_fmt: Format,
    tiling: Tiling,
    usage: Usage,
}

pub trait PhysicalDevice: Sealed {
    fn best_fmt(&self, desired_fmts: &[Format], tiling: Tiling, usage: Usage) -> Option<Format>;
    fn mem_ty(&self, type_mask: u32, properties: Properties) -> MemoryTypeId;
    fn queue_mut(&mut self) -> &mut <_Backend as Backend>::CommandQueue;
    fn gpu(&self) -> &<_Backend as Backend>::PhysicalDevice;
}
