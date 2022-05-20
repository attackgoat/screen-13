use {
    super::{DescriptorSetLayout, Device, DriverError, SamplerDesc, VertexInputState},
    archery::{SharedPointer, SharedPointerKind},
    ash::vk,
    derive_builder::Builder,
    log::{debug, error, info, trace},
    spirq::{
        ty::{ScalarType, Type},
        DescriptorType, EntryPoint, ReflectConfig, Variable,
    },
    std::{
        collections::{btree_map::BTreeMap, HashMap},
        fmt::{Debug, Formatter},
        iter::repeat,
    },
};

pub type DescriptorBindingMap = BTreeMap<DescriptorBinding, (DescriptorInfo, vk::ShaderStageFlags)>;

fn guess_immutable_sampler(
    device: &Device<impl SharedPointerKind>,
    binding_name: &str,
) -> vk::Sampler {
    const INVALID_ERR: &str = "Invalid sampler specification";

    let (texel_filter, mipmap_mode, address_modes) = if binding_name.contains("_sampler_") {
        let spec = &binding_name[binding_name.len() - 3..];
        let texel_filter = match &spec[0..1] {
            "n" => vk::Filter::NEAREST,
            "l" => vk::Filter::LINEAR,
            _ => panic!("{INVALID_ERR}: {}", &spec[0..1]),
        };

        let mipmap_mode = match &spec[1..2] {
            "n" => vk::SamplerMipmapMode::NEAREST,
            "l" => vk::SamplerMipmapMode::LINEAR,
            _ => panic!("{INVALID_ERR}: {}", &spec[1..2]),
        };

        let address_modes = match &spec[2..3] {
            "b" => vk::SamplerAddressMode::CLAMP_TO_BORDER,
            "e" => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            "m" => vk::SamplerAddressMode::MIRRORED_REPEAT,
            "r" => vk::SamplerAddressMode::REPEAT,
            _ => panic!("{INVALID_ERR}: {}", &spec[2..3]),
        };

        (texel_filter, mipmap_mode, address_modes)
    } else {
        debug!("image binding {binding_name} using default sampler");

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

/// Tuple of descriptor set index and binding index.
///
/// This is a generic representation of the descriptor binding point within the shader and not a
/// bound descriptor reference.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DescriptorBinding(pub u32, pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DescriptorInfo {
    AccelerationStructure(u32),
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
    pub fn binding_count(self) -> u32 {
        match self {
            Self::AccelerationStructure(binding_count) => binding_count,
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
            DescriptorInfo::AccelerationStructure(_) => Self::ACCELERATION_STRUCTURE_KHR,
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
        device: &SharedPointer<Device<P>, P>,
        descriptor_bindings: &DescriptorBindingMap,
    ) -> Result<Self, DriverError>
    where
        P: SharedPointerKind,
    {
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

            for (descriptor_binding, &(descriptor_info, stage_flags)) in descriptor_bindings
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
#[builder(build_fn(private, name = "fallible_build"), pattern = "owned")]
pub struct Shader {
    #[builder(default = "\"main\".to_owned()")]
    pub entry_name: String,

    #[builder(default, setter(strip_option))]
    pub specialization_info: Option<SpecializationInfo>,

    pub spirv: Vec<u8>,
    pub stage: vk::ShaderStageFlags,

    entry_point: EntryPoint,
}

impl Shader {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(stage: vk::ShaderStageFlags, spirv: impl ShaderCode) -> ShaderBuilder {
        ShaderBuilder::default()
            .spirv(spirv.into_vec())
            .stage(stage)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_any_hit(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::ANY_HIT_KHR, spirv)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_callable(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::CALLABLE_KHR, spirv)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_closest_hit(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::CLOSEST_HIT_KHR, spirv)
    }

    /// Creates a new compute shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_compute(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::COMPUTE, spirv)
    }

    /// Creates a new fragment shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_fragment(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::FRAGMENT, spirv)
    }

    /// Creates a new geometry shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_geometry(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::GEOMETRY, spirv)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_intersection(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::INTERSECTION_KHR, spirv)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_miss(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::MISS_KHR, spirv)
    }

    /// Creates a new ray trace shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_ray_gen(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::RAYGEN_KHR, spirv)
    }

    /// Creates a new tesselation control shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_tesselation_ctrl(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::TESSELLATION_CONTROL, spirv)
    }

    /// Creates a new tesselation evaluation shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_tesselation_eval(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::TESSELLATION_EVALUATION, spirv)
    }

    /// Creates a new vertex shader.
    ///
    /// _NOTE:_ May panic if the shader code is invalid.
    pub fn new_vertex(spirv: impl ShaderCode) -> ShaderBuilder {
        Self::new(vk::ShaderStageFlags::VERTEX, spirv)
    }

    /// Returns the input and write attachments of a shader.
    pub fn attachments(
        &self,
    ) -> (
        impl Iterator<Item = u32> + '_,
        impl Iterator<Item = u32> + '_,
    ) {
        (
            self.entry_point.vars.iter().filter_map(|var| match var {
                Variable::Descriptor {
                    desc_ty: DescriptorType::InputAttachment(attachment),
                    ..
                } => Some(*attachment),
                _ => None,
            }),
            self.entry_point.vars.iter().filter_map(|var| match var {
                Variable::Output { location, .. } => Some(location.loc()),
                _ => None,
            }),
        )
    }

    pub fn descriptor_bindings(
        &self,
        device: &Device<impl SharedPointerKind>,
    ) -> DescriptorBindingMap {
        let mut res = DescriptorBindingMap::default();

        for (name, binding, desc_ty, binding_count) in
            self.entry_point.vars.iter().filter_map(|var| match var {
                Variable::Descriptor {
                    name,
                    desc_bind,
                    desc_ty,
                    nbind,
                    ..
                } => Some((name, desc_bind, desc_ty, *nbind)),
                _ => None,
            })
        {
            trace!(
                "binding {}: {}.{} = {:?}[{}]",
                name.as_deref().unwrap_or_default(),
                binding.set(),
                binding.bind(),
                *desc_ty,
                binding_count
            );

            let descriptor_info = match desc_ty {
                DescriptorType::AccelStruct() => {
                    DescriptorInfo::AccelerationStructure(binding_count)
                }
                DescriptorType::CombinedImageSampler() => DescriptorInfo::CombinedImageSampler(
                    binding_count,
                    guess_immutable_sampler(device, name.as_deref().expect("invalid binding name")),
                ),
                DescriptorType::InputAttachment(attachment) => {
                    DescriptorInfo::InputAttachment(binding_count, *attachment)
                }
                DescriptorType::SampledImage() => DescriptorInfo::SampledImage(binding_count),
                DescriptorType::Sampler() => DescriptorInfo::Sampler(binding_count),
                DescriptorType::StorageBuffer(_access_ty) => {
                    DescriptorInfo::StorageBuffer(binding_count)
                }
                DescriptorType::StorageImage(_access_ty) => {
                    DescriptorInfo::StorageImage(binding_count)
                }
                DescriptorType::StorageTexelBuffer(_access_ty) => {
                    DescriptorInfo::StorageTexelBuffer(binding_count)
                }
                DescriptorType::UniformBuffer() => DescriptorInfo::UniformBuffer(binding_count),
                DescriptorType::UniformTexelBuffer() => {
                    DescriptorInfo::UniformTexelBuffer(binding_count)
                }
            };
            res.insert(
                DescriptorBinding(binding.set(), binding.bind()),
                (descriptor_info, self.stage),
            );
        }

        res
    }

    pub fn merge_descriptor_bindings(
        descriptor_bindings: impl IntoIterator<Item = DescriptorBindingMap>,
    ) -> DescriptorBindingMap {
        fn merge_info(lhs: &mut DescriptorInfo, rhs: DescriptorInfo) {
            // All references to a descriptor in a pipeline must be of the same type across all
            // pipeline stages
            const INVALID_ERR: &str = "invalid shader descriptor type";

            match lhs {
                DescriptorInfo::AccelerationStructure(lhs) => {
                    if let DescriptorInfo::AccelerationStructure(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::CombinedImageSampler(lhs, lhs_sampler) => {
                    if let DescriptorInfo::CombinedImageSampler(rhs, rhs_sampler) = rhs {
                        // Allow one of the samplers to be null (only one!)
                        if *lhs_sampler == vk::Sampler::null() {
                            *lhs_sampler = rhs_sampler;
                        }

                        debug_assert_ne!(*lhs_sampler, vk::Sampler::null());

                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::InputAttachment(lhs, lhs_idx) => {
                    if let DescriptorInfo::InputAttachment(rhs, rhs_idx) = rhs {
                        debug_assert_eq!(*lhs_idx, rhs_idx);

                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::SampledImage(lhs) => {
                    if let DescriptorInfo::SampledImage(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::Sampler(lhs) => {
                    if let DescriptorInfo::Sampler(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::StorageBuffer(lhs) => {
                    if let DescriptorInfo::StorageBuffer(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::StorageBufferDynamic(lhs) => {
                    if let DescriptorInfo::StorageBufferDynamic(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::StorageImage(lhs) => {
                    if let DescriptorInfo::StorageImage(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::StorageTexelBuffer(lhs) => {
                    if let DescriptorInfo::StorageTexelBuffer(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::UniformBuffer(lhs) => {
                    if let DescriptorInfo::UniformBuffer(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::UniformBufferDynamic(lhs) => {
                    if let DescriptorInfo::UniformBufferDynamic(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
                DescriptorInfo::UniformTexelBuffer(lhs) => {
                    if let DescriptorInfo::UniformTexelBuffer(rhs) = rhs {
                        *lhs = rhs.max(*lhs);
                    } else {
                        panic!("{INVALID_ERR}");
                    }
                }
            }
        }

        fn merge_pair(src: DescriptorBindingMap, dst: &mut DescriptorBindingMap) {
            for (descriptor_binding, (descriptor_info, descriptor_flags)) in src.into_iter() {
                if let Some((existing_info, existing_flags)) = dst.get_mut(&descriptor_binding) {
                    merge_info(existing_info, descriptor_info);
                    *existing_flags |= descriptor_flags;
                } else {
                    dst.insert(descriptor_binding, (descriptor_info, descriptor_flags));
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

    pub fn push_constant_range(&self) -> Option<vk::PushConstantRange> {
        self.entry_point
            .vars
            .iter()
            .filter_map(|var| match var {
                Variable::PushConstant {
                    ty: Type::Struct(ty),
                    ..
                } => Some(ty.members.clone()),
                _ => None,
            })
            .flatten()
            .map(|push_const| {
                push_const.offset..push_const.offset + push_const.ty.nbyte().unwrap_or_default()
            })
            .reduce(|a, b| a.start.min(b.start)..a.end.max(b.end))
            .map(|push_const| vk::PushConstantRange {
                stage_flags: self.stage,
                size: (push_const.end - push_const.start) as _,
                offset: push_const.start as _,
            })
    }

    fn reflect_entry_point(
        entry_name: &str,
        spirv: &[u8],
        specialization_info: Option<&SpecializationInfo>,
    ) -> Result<EntryPoint, DriverError> {
        let mut config = ReflectConfig::new();
        config.ref_all_rscs(true).spv(spirv);

        if let Some(spec_info) = specialization_info {
            for spec in &spec_info.map_entries {
                config.specialize(
                    spec.constant_id,
                    spec_info.data[spec.offset as usize..spec.offset as usize + spec.size]
                        .try_into()
                        .map_err(|_| DriverError::InvalidData)?,
                );
            }
        }

        let entry_points = config.reflect().map_err(|_| {
            error!("Unable to reflect spirv");

            DriverError::InvalidData
        })?;
        let entry_point = entry_points
            .into_iter()
            .find(|entry_point| entry_point.name == entry_name)
            .ok_or_else(|| {
                error!("Entry point not found");

                DriverError::InvalidData
            })?;

        Ok(entry_point)
    }

    pub fn vertex_input(&self) -> VertexInputState {
        fn scalar_format(ty: &ScalarType, byte_len: u32) -> vk::Format {
            match ty {
                ScalarType::Float(_) => match byte_len {
                    4 => vk::Format::R32_SFLOAT,
                    8 => vk::Format::R32G32_SFLOAT,
                    12 => vk::Format::R32G32B32_SFLOAT,
                    16 => vk::Format::R32G32B32A32_SFLOAT,
                    _ => unimplemented!("byte_len {byte_len}"),
                },
                ScalarType::Signed(_) => match byte_len {
                    4 => vk::Format::R32_SINT,
                    8 => vk::Format::R32G32_SINT,
                    12 => vk::Format::R32G32B32_SINT,
                    16 => vk::Format::R32G32B32A32_SINT,
                    _ => unimplemented!("byte_len {byte_len}"),
                },
                ScalarType::Unsigned(_) => match byte_len {
                    4 => vk::Format::R32_UINT,
                    8 => vk::Format::R32G32_UINT,
                    12 => vk::Format::R32G32B32_UINT,
                    16 => vk::Format::R32G32B32A32_UINT,
                    _ => unimplemented!("byte_len {byte_len}"),
                },
                _ => unimplemented!("{:?}", ty),
            }
        }

        let mut input_rates_strides = HashMap::new();
        let mut vertex_attribute_descriptions = vec![];

        for (name, location, ty) in self.entry_point.vars.iter().filter_map(|var| match var {
            Variable::Input { name, location, ty } => Some((name, location, ty)),
            _ => None,
        }) {
            let (binding, guessed_rate) = name
                .as_ref()
                .filter(|name| name.contains("_ibind") || name.contains("_vbind"))
                .map(|name| {
                    let binding = name[name.rfind("bind").unwrap()..]
                        .parse()
                        .unwrap_or_default();
                    let rate = if name.contains("_ibind") {
                        vk::VertexInputRate::INSTANCE
                    } else {
                        vk::VertexInputRate::VERTEX
                    };

                    (binding, rate)
                })
                .unwrap_or_default();
            let (location, _) = location.into_inner();
            if let Some((input_rate, _)) = input_rates_strides.get(&binding) {
                assert_eq!(*input_rate, guessed_rate);
            }

            let byte_stride = ty.nbyte().unwrap_or_default() as u32;
            let (input_rate, stride) = input_rates_strides.entry(binding).or_default();
            *input_rate = guessed_rate;
            *stride += byte_stride;

            //trace!("{location} {:?} is {byte_stride} bytes", name);

            vertex_attribute_descriptions.push(vk::VertexInputAttributeDescription {
                location,
                binding,
                format: match ty {
                    Type::Scalar(ty) => scalar_format(ty, ty.nbyte() as _),
                    Type::Vector(ty) => scalar_format(&ty.scalar_ty, byte_stride),
                    _ => unimplemented!("{:?}", ty),
                },
                offset: byte_stride, // Figured out below - this data is iter'd in an unknown order
            });
        }

        vertex_attribute_descriptions.sort_unstable_by(|lhs, rhs| {
            let binding = lhs.binding.cmp(&rhs.binding);
            if binding.is_lt() {
                return binding;
            }

            lhs.location.cmp(&rhs.location)
        });

        let mut offset = 0;
        let mut offset_binding = 0;

        for vertex_attribute_description in &mut vertex_attribute_descriptions {
            if vertex_attribute_description.binding != offset_binding {
                offset_binding = vertex_attribute_description.binding;
                offset = 0;
            }

            let stride = vertex_attribute_description.offset;
            vertex_attribute_description.offset = offset;
            offset += stride;

            info!(
                "vertex attribute {}.{}: {:?} (offset={})",
                vertex_attribute_description.binding,
                vertex_attribute_description.location,
                vertex_attribute_description.format,
                vertex_attribute_description.offset,
            );
        }

        let mut vertex_binding_descriptions = vec![];
        for (binding, (input_rate, stride)) in input_rates_strides.into_iter() {
            vertex_binding_descriptions.push(vk::VertexInputBindingDescription {
                binding,
                input_rate,
                stride,
            });
        }

        VertexInputState {
            vertex_attribute_descriptions,
            vertex_binding_descriptions,
        }
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl ShaderBuilder {
    pub fn new(stage: vk::ShaderStageFlags, spirv: Vec<u8>) -> Self {
        Self::default().stage(stage).spirv(spirv)
    }

    pub fn build(mut self) -> Shader {
        self.entry_point = Some(
            Shader::reflect_entry_point(
                self.entry_name.as_deref().unwrap_or("main"),
                self.spirv.as_deref().unwrap(),
                self.specialization_info
                    .as_ref()
                    .map(|opt| opt.as_ref())
                    .unwrap_or_default(),
            )
            .expect("invalid shader code"),
        );

        self.fallible_build()
            .expect("All required fields set at initialization")
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
        shader.build()
    }
}

pub trait ShaderCode {
    fn into_vec(self) -> Vec<u8>;
}

impl ShaderCode for &[u8] {
    fn into_vec(self) -> Vec<u8> {
        debug_assert_eq!(self.len() % 4, 0, "invalid spir-v code");

        self.to_vec()
    }
}

impl ShaderCode for &[u32] {
    fn into_vec(self) -> Vec<u8> {
        pub fn into_u8_slice<T>(t: &[T]) -> &[u8]
        where
            T: Sized,
        {
            use std::{mem::size_of, slice::from_raw_parts};

            unsafe { from_raw_parts(t.as_ptr() as *const _, t.len() * size_of::<T>()) }
        }

        into_u8_slice(self).into_vec()
    }
}

impl ShaderCode for Vec<u8> {
    fn into_vec(self) -> Vec<u8> {
        debug_assert_eq!(self.len() % 4, 0, "invalid spir-v code");

        self
    }
}

impl ShaderCode for Vec<u32> {
    fn into_vec(self) -> Vec<u8> {
        self.as_slice().into_vec()
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
