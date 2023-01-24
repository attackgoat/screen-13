//! Vertex layout types

pub use proc_macros::Vertex;
use {
    screen_13::driver::{
        ash::vk,
        graphic::VertexInputState,
        shader::{Shader, ShaderBuilder},
        DriverError,
    },
    spirq::Variable,
    std::collections::HashMap,
};

// TODO: maybe return tuple instead of `VertexInputState`
// TODO: infer block-size from format
// TODO: make sure doubles are handled correctly
// TODO: validate as much as sensible
// TODO: make sure derive macro handles padding fields

pub trait ShaderBuilderExt {
    fn with_vertex_layout(self, layout: impl VertexLayout) -> Shader;
}

impl ShaderBuilderExt for ShaderBuilder {
    #[inline]
    fn with_vertex_layout(self, layout: impl VertexLayout) -> Shader {
        let mut shader = self.build();

        let entry_point = Shader::reflect_entry_point(
            &shader.entry_name,
            &shader.spirv,
            shader.specialization_info.as_ref(),
        )
        .unwrap(); // TODO: expect

        let state = layout.specialize(&entry_point.vars).unwrap(); // expect
        shader.vertex_input_state = Some(state);
        shader
    }
}

/// Trait used to specialize an defined VertexLayouts.
pub trait VertexLayout {
    /// Creates a VertexInputState from the VertexLayout and a set of shader-variables.
    fn specialize(&self, inputs: &[Variable]) -> Result<VertexInputState, DriverError>;
}

impl VertexLayout for VertexInputState {
    #[inline]
    fn specialize(&self, _inputs: &[Variable]) -> Result<VertexInputState, DriverError> {
        Ok(self.clone())
    }
}

impl<T> VertexLayout for &[T]
where
    T: VertexLayout,
{
    #[inline]
    fn specialize(&self, inputs: &[Variable]) -> Result<VertexInputState, DriverError> {
        let mut states = Vec::with_capacity(self.len());
        // When merging states, we need to keep track of current bindings
        let mut curr_binding = 0;

        for vertex in self.iter() {
            let mut state = vertex.specialize(inputs)?;

            for binding in state.vertex_binding_descriptions.iter_mut() {
                binding.binding += curr_binding;
            }
            for attribute in state.vertex_attribute_descriptions.iter_mut() {
                attribute.binding += curr_binding;
            }

            curr_binding += state.vertex_binding_descriptions.len() as u32;
            states.push(state);
        }

        Ok(VertexInputState {
            vertex_binding_descriptions: states
                .clone() // TODO: avoid this clone, maybe unzip?
                .into_iter()
                .flat_map(|state| state.vertex_binding_descriptions)
                .collect(),
            vertex_attribute_descriptions: states
                .into_iter()
                .flat_map(|state| state.vertex_attribute_descriptions)
                .collect(),
        })
    }
}

impl<T, const N: usize> VertexLayout for [T; N]
where
    T: VertexLayout,
{
    #[inline]
    fn specialize(&self, inputs: &[Variable]) -> Result<VertexInputState, DriverError> {
        self.as_slice().specialize(inputs)
    }
}

impl<T> VertexLayout for Vec<T>
where
    T: VertexLayout,
{
    #[inline]
    fn specialize(&self, inputs: &[Variable]) -> Result<VertexInputState, DriverError> {
        self.as_slice().specialize(inputs)
    }
}

pub trait Vertex {
    fn layout(input_rate: vk::VertexInputRate) -> DerivedVertexLayout;
}

pub struct DerivedVertexLayout {
    pub attributes: HashMap<String, DerivedVertexAttribute>,
    pub stride: u32,
    pub input_rate: vk::VertexInputRate,
}

pub struct DerivedVertexAttribute {
    pub offset: u32,
    pub offset_inc: u32,
    pub format: vk::Format,
    pub num_locations: u32,
}

impl VertexLayout for DerivedVertexLayout {
    #[inline]
    fn specialize(&self, inputs: &[Variable]) -> Result<VertexInputState, DriverError> {
        let bindings = vec![vk::VertexInputBindingDescription {
            binding: 0,
            stride: self.stride,
            input_rate: self.input_rate,
        }];
        let attributes = inputs
            .iter()
            .filter_map(|var| match var {
                Variable::Input {
                    name,
                    location,
                    ty: _,
                } => self
                    .attributes
                    .get(&name.to_owned().unwrap_or("".to_string()))
                    .map(|attribute| {
                        (location.loc()..location.loc() + attribute.num_locations)
                            .enumerate()
                            .map(|(i, location)| vk::VertexInputAttributeDescription {
                                binding: 0,
                                location,
                                format: attribute.format,
                                offset: attribute.offset + i as u32 * attribute.offset_inc,
                            })
                    }),
                _ => None,
            })
            .flatten()
            .collect();
        Ok(VertexInputState {
            vertex_binding_descriptions: bindings,
            vertex_attribute_descriptions: attributes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Vertex, VertexInputState, VertexLayout};
    use screen_13::driver::ash::vk;
    use spirq::{ty::Type, InterfaceLocation, Variable};

    #[test]
    fn vertex_input() {
        let state = VertexInputState {
            vertex_binding_descriptions: vec![vk::VertexInputBindingDescription {
                binding: 0,
                stride: 0,
                input_rate: vk::VertexInputRate::VERTEX,
            }],
            vertex_attribute_descriptions: vec![vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32_SFLOAT,
                offset: 0,
            }],
        };
        let inputs = [Variable::Input {
            name: Some("name".to_owned()),
            location: InterfaceLocation::new(0, 0),
            ty: Type::Scalar(spirq::ty::ScalarType::Float(1)),
        }];
        let output = [state.clone(), state].specialize(&inputs).unwrap();
        assert_eq!(output.vertex_binding_descriptions.len(), 2);
        assert_eq!(output.vertex_attribute_descriptions.len(), 2);
        // Let's check if bindings were incremented correctly
        assert_eq!(output.vertex_binding_descriptions[0].binding, 0);
        assert_eq!(output.vertex_binding_descriptions[1].binding, 1);
        assert_eq!(output.vertex_attribute_descriptions[0].binding, 0);
        assert_eq!(output.vertex_attribute_descriptions[1].binding, 1);
    }

    #[test]
    fn derive_vertex() {
        #[repr(C)]
        #[derive(Vertex)]
        struct MyVertex {
            #[format(R16G16B16_SNORM)]
            normal: [i32; 3],
            #[name("in_proj", "cam_proj")]
            #[format(R32G32B32A32_SFLOAT, 4)]
            proj: [f32; 16],
        }
        let layout = MyVertex::layout(vk::VertexInputRate::VERTEX);
        assert_eq!(layout.attributes.len(), 3);
        let inputs = [Variable::Input {
            name: Some("in_proj".to_string()),
            location: InterfaceLocation::new(0, 0),
            ty: Type::Scalar(spirq::ty::ScalarType::Float(1)), // unused in impl
        }];
        let state = layout.specialize(&inputs).unwrap();
        let bindings = &state.vertex_binding_descriptions;
        let attributes = &state.vertex_attribute_descriptions;
        assert_eq!(bindings.len(), 1);
        assert_eq!(attributes.len(), 4);
        // Let's check if the offset for the multiple locations for the array was incremented
        // correctly:
        assert_eq!(attributes[0].offset, 12);
        assert_eq!(attributes[1].offset, 28);
        assert_eq!(attributes[2].offset, 44);
        assert_eq!(attributes[3].offset, 60);
    }
}
