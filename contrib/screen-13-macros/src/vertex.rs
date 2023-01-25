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

// TODO: validate as much as sensible

pub trait ShaderBuilderExt {
    fn with_vertex_layout(self, layout: impl VertexLayout) -> Shader;
}

impl ShaderBuilderExt for ShaderBuilder {
    #[inline]
    fn with_vertex_layout(self, layout: impl VertexLayout) -> Shader {
        let mut shader = self.build();

        let state = layout.specialize(&shader.entry_point().vars).unwrap(); // expect
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
    // The block-size of the format. The derive-macro will calculate it by dividing
    // `size_of::<T>()` by `num_locations`.
    pub block_size: u32,
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
                        // Formats such as R64G64B64A64_SFLOAT exceed the 16-byte limit of a location and therefore require two
                        let locations = if attribute.block_size > 16 {
                            (location.loc()..location.loc() + 2 * attribute.num_locations)
                                .step_by(2)
                        } else {
                            (location.loc()..location.loc() + attribute.num_locations).step_by(1)
                        };
                        locations.enumerate().map(|(i, location)| {
                            vk::VertexInputAttributeDescription {
                                binding: 0,
                                location,
                                format: attribute.format,
                                offset: attribute.offset + i as u32 * attribute.block_size,
                            }
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
            _padding: u32,
            #[name("in_proj", "cam_proj")]
            #[format(R32G32B32A32_SFLOAT, 4)]
            proj: [f32; 16],
            #[format(R64G64B64A64_SFLOAT, 2)]
            double: [f64; 8],
        }
        let layout = MyVertex::layout(vk::VertexInputRate::VERTEX);
        assert_eq!(layout.attributes.len(), 4);
        // Only contains "in_proj" and "double"
        let inputs = [
            Variable::Input {
                name: Some("in_proj".to_string()),
                location: InterfaceLocation::new(0, 0),
                ty: Type::Scalar(spirq::ty::ScalarType::Float(1)), // unused in impl
            },
            Variable::Input {
                name: Some("double".to_string()),
                location: InterfaceLocation::new(1, 0),
                ty: Type::Scalar(spirq::ty::ScalarType::Float(1)), // unused in impl
            },
        ];
        let state = layout.specialize(&inputs).unwrap();
        let bindings = &state.vertex_binding_descriptions;
        let attributes = &state.vertex_attribute_descriptions;
        assert_eq!(bindings.len(), 1);
        assert_eq!(attributes.len(), 6);
        // Let's check if the offset for the multiple locations for the array was incremented
        // correctly:
        assert_eq!(attributes[0].offset, 16);
        assert_eq!(attributes[1].offset, 32);
        assert_eq!(attributes[2].offset, 48);
        assert_eq!(attributes[3].offset, 64);
        // Let's check if the location was incremented by two for double
        assert_eq!(attributes[4].location, 1);
        assert_eq!(attributes[5].location, 3);
    }
}
