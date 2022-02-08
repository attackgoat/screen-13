use {
    glam::Quat,
    gltf::animation::Interpolation,
    serde::{Deserialize, Serialize},
};

/// Holds an `Animation` in a `.pak` file. For data transport only.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct AnimationBuf {
    /// The channels (joints/bones) of movement used in this `Animation`.
    pub channels: Vec<Channel>,
}

/// Describes the animation of one joint.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Channel {
    inputs: Vec<f32>,
    interpolation: Interpolation,
    rotations: Vec<Quat>,
    target: String,
}

impl Channel {
    #[allow(unused)]
    pub(crate) fn new<T: AsRef<str>, I: IntoIterator<Item = f32>, R: IntoIterator<Item = Quat>>(
        target: T,
        interpolation: Interpolation,
        inputs: I,
        rotations: R,
    ) -> Self {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let rotations = rotations.into_iter().collect::<Vec<_>>();
        let target = target.as_ref().to_owned();

        assert!(!target.is_empty());
        assert_ne!(inputs.len(), 0);

        match interpolation {
            Interpolation::Linear | Interpolation::Step => {
                assert_eq!(inputs.len(), rotations.len());
            }
            Interpolation::CubicSpline => {
                assert_eq!(inputs.len() * 3, rotations.len());
            }
        }

        Self {
            inputs,
            interpolation,
            rotations,
            target,
        }
    }

    /// The target joint/bone.
    pub fn target(&self) -> &str {
        &self.target
    }
}
