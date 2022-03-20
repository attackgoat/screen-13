use {
    super::{DescriptorSetLayout, Device, DriverError, SamplerDesc},
    crate::{as_u32_slice, ptr::Shared},
    archery::SharedPointerKind,
    ash::vk,
    derive_builder::Builder,
    log::{trace, warn},
    spirv_reflect::{create_shader_module, types::ReflectDescriptorType, ShaderModule},
    std::{
        collections::btree_map::BTreeMap,
        fmt::{Debug, Formatter},
        iter::repeat,
        ops::Deref,
    },
};

pub type DescriptorBindingMap = BTreeMap<DescriptorBinding, DescriptorInfo>;

fn guess_immutable_sampler(
    device: &Device<impl SharedPointerKind>,
    binding_name: &str,
) -> Option<vk::Sampler> {
    let (texel_filter, mipmap_mode, address_modes) = if binding_name.contains("_sampler_") {
        let spec = &binding_name[binding_name.len() - 3..];
        let texel_filter = match &spec[0..1] {
            "n" => vk::Filter::NEAREST,
            "l" => vk::Filter::LINEAR,
            _ => panic!("{}", &spec[0..1]),
        };

        let mipmap_mode = match &spec[1..2] {
            "n" => vk::SamplerMipmapMode::NEAREST,
            "l" => vk::SamplerMipmapMode::LINEAR,
            _ => panic!("{}", &spec[1..2]),
        };

        let address_modes = match &spec[2..3] {
            "b" => vk::SamplerAddressMode::CLAMP_TO_BORDER,
            "e" => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            "m" => vk::SamplerAddressMode::MIRRORED_REPEAT,
            "r" => vk::SamplerAddressMode::REPEAT,
            _ => panic!("{}", &spec[2..3]),
        };

        (texel_filter, mipmap_mode, address_modes)
    } else {
        (
            vk::Filter::LINEAR,
            vk::SamplerMipmapMode::LINEAR,
            vk::SamplerAddressMode::REPEAT,
        )
    };

    Device::immutable_sampler(
        device,
        SamplerDesc {
            texel_filter,
            mipmap_mode,
            address_modes,
        },
    )
}

/// Set index and binding index - this is a generic representation of the descriptor binding point
/// within the shader and not a bound descriptor reference.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DescriptorBinding(pub u32, pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DescriptorInfo {
    AccelerationStructureNV(u32),
    CombinedImageSampler(u32, vk::Sampler),
    InputAttachment(u32, u32), //count, input index,
    SampledImage(u32),
    Sampler(u32),
    StorageBuffer(u32),
    StorageBufferDynamic(u32),
    StorageImage(u32),
    StorageTexelBuffer(u32),
    UniformBuffer(u32),
    UniformBufferDynamic(u32),
    UniformTexelBuffer(u32),
}

impl DescriptorInfo {
    fn binding_count(self) -> u32 {
        match self {
            Self::AccelerationStructureNV(binding_count) => binding_count,
            Self::CombinedImageSampler(binding_count, _) => binding_count,
            Self::InputAttachment(binding_count, _) => binding_count,
            Self::SampledImage(binding_count) => binding_count,
            Self::Sampler(binding_count) => binding_count,
            Self::StorageBuffer(binding_count) => binding_count,
            Self::StorageBufferDynamic(binding_count) => binding_count,
            Self::StorageImage(binding_count) => binding_count,
            Self::StorageTexelBuffer(binding_count) => binding_count,
            Self::UniformBuffer(binding_count) => binding_count,
            Self::UniformBufferDynamic(binding_count) => binding_count,
            Self::UniformTexelBuffer(binding_count) => binding_count,
        }
    }

    pub fn sampler(self) -> Option<vk::Sampler> {
        match self {
            Self::CombinedImageSampler(_, sampler) => Some(sampler),
            _ => None,
        }
    }
}

impl From<DescriptorInfo> for vk::DescriptorType {
    fn from(descriptor_info: DescriptorInfo) -> Self {
        match descriptor_info {
            DescriptorInfo::AccelerationStructureNV(_) => Self::ACCELERATION_STRUCTURE_NV,
            DescriptorInfo::CombinedImageSampler(..) => Self::COMBINED_IMAGE_SAMPLER,
            DescriptorInfo::InputAttachment(..) => Self::INPUT_ATTACHMENT,
            DescriptorInfo::SampledImage(_) => Self::SAMPLED_IMAGE,
            DescriptorInfo::Sampler(_) => Self::SAMPLER,
            DescriptorInfo::StorageBuffer(_) => Self::STORAGE_BUFFER,
            DescriptorInfo::StorageBufferDynamic(_) => Self::STORAGE_BUFFER_DYNAMIC,
            DescriptorInfo::StorageImage(_) => Self::STORAGE_IMAGE,
            DescriptorInfo::StorageTexelBuffer(_) => Self::STORAGE_TEXEL_BUFFER,
            DescriptorInfo::UniformBuffer(_) => Self::UNIFORM_BUFFER,
            DescriptorInfo::UniformBufferDynamic(_) => Self::UNIFORM_BUFFER_DYNAMIC,
            DescriptorInfo::UniformTexelBuffer(_) => Self::UNIFORM_TEXEL_BUFFER,
        }
    }
}

#[derive(Debug)]
pub struct PipelineDescriptorInfo<P>
where
    P: SharedPointerKind,
{
    pub layouts: BTreeMap<u32, DescriptorSetLayout<P>>,
    pub pool_sizes: BTreeMap<u32, BTreeMap<vk::DescriptorType, u32>>,
}

impl<P> PipelineDescriptorInfo<P>
where
    P: SharedPointerKind,
{
    pub fn create(
        device: &Shared<Device<P>, P>,
        descriptor_bindings: &DescriptorBindingMap,
        stage_flags: vk::ShaderStageFlags,
    ) -> Result<Self, DriverError>
    where
        P: SharedPointerKind,
    {
        use std::slice::from_ref;

        let descriptor_set_count = descriptor_bindings
            .keys()
            .max()
            .copied()
            .map(|descriptor_binding| descriptor_binding.0 + 1)
            .unwrap_or_default();
        let mut layouts = BTreeMap::new();
        let mut pool_sizes = BTreeMap::new();

        // trace!("descriptor_bindings: {:#?}", &descriptor_bindings);

        for descriptor_set_idx in 0..descriptor_set_count {
            // HACK: We need to keep the immutable samplers alive until create, could be cleaner..
            let mut immutable_samplers = vec![];
            let mut binding_counts = BTreeMap::<vk::DescriptorType, u32>::new();
            let mut bindings = vec![];

            for (descriptor_binding, &descriptor_info) in descriptor_bindings
                .iter()
                .filter(|(descriptor_binding, _)| descriptor_binding.0 == descriptor_set_idx)
            {
                let descriptor_ty: vk::DescriptorType = descriptor_info.into();
                *binding_counts.entry(descriptor_ty).or_default() +=
                    descriptor_info.binding_count();
                let mut binding = vk::DescriptorSetLayoutBinding::builder()
                    .binding(descriptor_binding.1)
                    .descriptor_count(descriptor_info.binding_count())
                    .descriptor_type(descriptor_ty)
                    .stage_flags(stage_flags);

                if let Some(sampler) = descriptor_info.sampler() {
                    let start = immutable_samplers.len();
                    immutable_samplers
                        .extend(repeat(sampler).take(descriptor_info.binding_count() as _));
                    binding = binding.immutable_samplers(&immutable_samplers[start..]);
                }

                bindings.push(binding.build());
            }

            let pool_size = pool_sizes
                .entry(descriptor_set_idx)
                .or_insert_with(BTreeMap::new);

            for (descriptor_ty, binding_count) in binding_counts.into_iter() {
                *pool_size.entry(descriptor_ty).or_default() += binding_count;
            }

            // trace!("bindings: {:#?}", &bindings);

            let create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(bindings.as_slice())
                .build();

            layouts.insert(
                descriptor_set_idx,
                DescriptorSetLayout::create(device, &create_info)?,
            );
        }

        // trace!("layouts {:#?}", &layouts);
        // trace!("pool_sizes {:#?}", &pool_sizes);

        Ok(Self {
            layouts,
            pool_sizes,
        })
    }
}

#[derive(Builder, Clone)]
#[builder(pattern = "owned")]
pub struct Shader {
    #[builder(default = "\"main\".to_owned()")]
    pub entry_name: String,
    #[builder(default, setter(strip_option))]
    pub specialization_info: Option<SpecializationInfo>,
    pub spirv: Vec<u8>,
    pub stage: vk::ShaderStageFlags,
}

impl Shader {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(stage: vk::ShaderStageFlags, spirv: impl Into<Vec<u8>>) -> ShaderBuilder {
        ShaderBuilder::default().spirv(spirv.into()).stage(stage)
    }

    pub fn new_compute(spirv: impl Into<Vec<u8>>) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::COMPUTE, spirv)
    }

    pub fn new_fragment(spirv: impl Into<Vec<u8>>) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::FRAGMENT, spirv)
    }

    pub fn new_vertex(spirv: impl Into<Vec<u8>>) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::VERTEX, spirv)
    }

    pub fn descriptor_bindings(
        &self,
        device: &Device<impl SharedPointerKind>,
    ) -> Result<DescriptorBindingMap, DriverError> {
        let mut module = ShaderModule::load_u8_data(self.spirv.as_slice()).map_err(|_| {
            warn!("Unable to load shader module");

            DriverError::InvalidData
        })?;
        let descriptor_bindings = module
            .enumerate_descriptor_bindings(Some(&self.entry_name))
            .map_err(|_| {
                warn!("Unable to enumerate shader descriptor bindings");

                DriverError::InvalidData
            })?;
        let mut res = DescriptorBindingMap::default();

        for binding in descriptor_bindings {
            res.insert(
                DescriptorBinding(binding.set, binding.binding),
                match binding.descriptor_type {
                    ReflectDescriptorType::AccelerationStructureNV => {
                        DescriptorInfo::AccelerationStructureNV(binding.count)
                    }
                    ReflectDescriptorType::CombinedImageSampler => {
                        DescriptorInfo::CombinedImageSampler(
                            binding.count,
                            guess_immutable_sampler(device, &binding.name).unwrap(),
                        )
                    }
                    ReflectDescriptorType::InputAttachment => DescriptorInfo::InputAttachment(
                        binding.count,
                        binding.input_attachment_index,
                    ),
                    ReflectDescriptorType::SampledImage => {
                        DescriptorInfo::SampledImage(binding.count)
                    }
                    ReflectDescriptorType::Sampler => DescriptorInfo::Sampler(binding.count),
                    ReflectDescriptorType::StorageBuffer => {
                        DescriptorInfo::StorageBuffer(binding.count)
                    }
                    ReflectDescriptorType::StorageBufferDynamic => {
                        DescriptorInfo::StorageBufferDynamic(binding.count)
                    }
                    ReflectDescriptorType::StorageImage => {
                        DescriptorInfo::StorageImage(binding.count)
                    }
                    ReflectDescriptorType::StorageTexelBuffer => {
                        DescriptorInfo::StorageTexelBuffer(binding.count)
                    }
                    ReflectDescriptorType::UniformBuffer => {
                        DescriptorInfo::UniformBuffer(binding.count)
                    }
                    ReflectDescriptorType::UniformBufferDynamic => {
                        DescriptorInfo::UniformBufferDynamic(binding.count)
                    }
                    ReflectDescriptorType::UniformTexelBuffer => {
                        DescriptorInfo::UniformTexelBuffer(binding.count)
                    }
                    _ => unimplemented!(),
                },
            );
        }

        Ok(res)
    }

    pub fn merge_descriptor_bindings(
        descriptor_bindings: impl IntoIterator<Item = DescriptorBindingMap>,
    ) -> DescriptorBindingMap {
        fn merge_pair(src: DescriptorBindingMap, dst: &mut DescriptorBindingMap) {
            for (descriptor_binding, descriptor_info) in src.into_iter() {
                if let Some(existing) = dst.get_mut(&descriptor_binding) {
                    assert_eq!(*existing, descriptor_info);

                    *existing = descriptor_info;
                } else {
                    dst.insert(descriptor_binding, descriptor_info);
                }
            }
        }

        let mut descriptor_bindings = descriptor_bindings.into_iter();
        let mut res = descriptor_bindings.next().unwrap_or_default();
        for descriptor_binding in descriptor_bindings {
            merge_pair(descriptor_binding, &mut res);
        }

        res
    }

    pub fn push_constant_ranges(&self) -> Result<Vec<vk::PushConstantRange>, DriverError> {
        let mut module = ShaderModule::load_u8_data(self.spirv.as_slice())
            .map_err(|_| DriverError::InvalidData)?;
        let block_vars = module
            .enumerate_push_constant_blocks(Some(&self.entry_name))
            .map_err(|_| DriverError::InvalidData)?;
        let mut res = vec![];

        for block_var in &block_vars {
            res.push(
                vk::PushConstantRange::builder()
                    .stage_flags(self.stage)
                    .size(block_var.size)
                    .offset(block_var.offset)
                    .build(),
            );
        }

        Ok(res)
    }
}

impl Debug for Shader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // We don't want the default formatter bc vec u8
        // TODO: Better output message
        f.write_str("Shader")
    }
}

impl From<ShaderBuilder> for Shader {
    fn from(shader: ShaderBuilder) -> Self {
        shader.build().unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct SpecializationInfo {
    pub data: Vec<u8>,
    pub map_entries: Vec<vk::SpecializationMapEntry>,
}

impl SpecializationInfo {
    pub fn new(
        map_entries: impl Into<Vec<vk::SpecializationMapEntry>>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            data: data.into(),
            map_entries: map_entries.into(),
        }
    }
}
