//! Screen 13 offers a number of vertex formats for use loading models.
//!
//! Additional vertex formats will be released to support new features in the future.
//!
//! # Model Vertices
//!
//! There are two main flavors of model vertex:
//! - Normal (`POSITION` and `TEXCOORD` _only_)
//! - Skinned (for animation - includes `JOINTS` and `WEIGHTS` attributes)
//!
//! Further, vertexes may be specified in the standard manner mentioned above or using the
//! `Ex`-variety structs. Extended model vertices include the `NORMAL` and `TANGENT`
//! attributes which are normally calculated at runtime.
//!
//! _NOTE:_ Extended vertices are only needed if you want more control over the normal/tangent
//! generation process.

use {
    crate::math::{
        vec2, vec2_is_finite, vec3, vec3_is_finite, vec4, vec4_is_finite, Vec2, Vec3, Vec4,
    },
    std::cmp::Ordering,
};

/// Helpful alias for standard vertex strides as a single byte array.
pub type NormalVertexArray = [f32; 5];

/// Helpful alias for standard vertex strides as a individual byte arrays.
pub type NormalVertexArrays = ([f32; 3], [f32; 2]);

/// Helpful alias for standard vertex strides as a tuple of vecs.
pub type NormalVertexTuple = (Vec3, Vec2);

/// Helpful alias for extended vertex strides as a single byte array.
pub type NormalVertexExArray = [f32; 12];

/// Helpful alias for extended vertex strides as a individual byte arrays.
pub type NormalVertexExArrays = ([f32; 3], [f32; 3], [f32; 4], [f32; 2]);

/// Helpful alias for extended vertex strides as a tuple of vecs.
pub type NormalVertexExTuple = (Vec3, Vec3, Vec4, Vec2);

/// Helpful alias for standard skinned vertex strides as a single byte array.
pub type SkinVertexArray = [f32; 13];

/// Helpful alias for standard skinned vertex strides as a individual byte arrays.
pub type SkinVertexArrays = ([f32; 3], [f32; 4], [f32; 4], [f32; 2]);

/// Helpful alias for standard skinned vertex strides as a tuple of vecs.
pub type SkinVertexTuple = (Vec3, Vec4, Vec4, Vec2);

/// Helpful alias for extended skinned vertex strides as a single byte array.
pub type SkinVertexExArray = [f32; 20];

/// Helpful alias for extended skinned vertex strides as a individual byte arrays.
pub type SkinVertexExArrays = ([f32; 3], [f32; 3], [f32; 4], [f32; 4], [f32; 4], [f32; 2]);

/// Helpful alias for extended skinned vertex strides as a tuple of vecs.
pub type SkinVertexExTuple = (Vec3, Vec3, Vec4, Vec4, Vec4, Vec2);

/// Defines all supported vertex formats.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Vertex {
    /// The "easy" vertex. If you are not sure what you want, this is probably it.
    Normal(NormalVertex),

    /// Same as `Normal`, except that the additional vertex attributes (normal and tanget) are
    /// specified here as opposed to being calculated before use.
    ///
    /// Only needed if you want more control over the normal/tangent generation process.
    NormalEx(NormalVertexEx),

    /// Same as `Normal`, except that additional vertex attributes for joints and weights are
    /// added.
    Skin(SkinVertex),

    /// Like `NormalEx`, a combination of `Skin` and extra normal and tangent vertex attributes.
    ///
    /// Only needed if you want more control over the normal/tangent generation process.
    SkinEx(SkinVertexEx),
}

impl Vertex {
    /// Returns `true` if all fields are finite.
    pub fn is_finite(self) -> bool {
        match self {
            Self::Normal(vertex) => vertex.is_finite(),
            Self::NormalEx(vertex) => vertex.is_finite(),
            Self::Skin(vertex) => vertex.is_finite(),
            Self::SkinEx(vertex) => vertex.is_finite(),
        }
    }
}

impl From<NormalVertex> for Vertex {
    fn from(val: NormalVertex) -> Self {
        Self::Normal(val)
    }
}

impl From<&NormalVertex> for Vertex {
    fn from(val: &NormalVertex) -> Self {
        Self::Normal(*val)
    }
}

impl From<NormalVertexArray> for Vertex {
    fn from(val: NormalVertexArray) -> Self {
        val.into()
    }
}

impl From<NormalVertexArrays> for Vertex {
    fn from(val: NormalVertexArrays) -> Self {
        val.into()
    }
}

impl From<(Vec3, Vec2)> for Vertex {
    fn from(val: (Vec3, Vec2)) -> Self {
        val.into()
    }
}

impl From<NormalVertexEx> for Vertex {
    fn from(val: NormalVertexEx) -> Self {
        Self::NormalEx(val)
    }
}

impl From<&NormalVertexEx> for Vertex {
    fn from(val: &NormalVertexEx) -> Self {
        Self::NormalEx(*val)
    }
}

impl From<NormalVertexExArray> for Vertex {
    fn from(val: NormalVertexExArray) -> Self {
        val.into()
    }
}

impl From<NormalVertexExArrays> for Vertex {
    fn from(val: NormalVertexExArrays) -> Self {
        val.into()
    }
}

impl From<(Vec3, Vec3, Vec4, Vec2)> for Vertex {
    fn from(val: (Vec3, Vec3, Vec4, Vec2)) -> Self {
        val.into()
    }
}

impl From<SkinVertex> for Vertex {
    fn from(val: SkinVertex) -> Self {
        Self::Skin(val)
    }
}

impl From<&SkinVertex> for Vertex {
    fn from(val: &SkinVertex) -> Self {
        Self::Skin(*val)
    }
}

impl From<SkinVertexArray> for Vertex {
    fn from(val: SkinVertexArray) -> Self {
        val.into()
    }
}

impl From<SkinVertexArrays> for Vertex {
    fn from(val: SkinVertexArrays) -> Self {
        val.into()
    }
}

impl From<SkinVertexTuple> for Vertex {
    fn from(val: SkinVertexTuple) -> Self {
        val.into()
    }
}

impl From<SkinVertexEx> for Vertex {
    fn from(val: SkinVertexEx) -> Self {
        Self::SkinEx(val)
    }
}

impl From<&SkinVertexEx> for Vertex {
    fn from(val: &SkinVertexEx) -> Self {
        Self::SkinEx(*val)
    }
}

impl From<SkinVertexExArray> for Vertex {
    fn from(val: SkinVertexExArray) -> Self {
        val.into()
    }
}

impl From<SkinVertexExArrays> for Vertex {
    fn from(val: SkinVertexExArrays) -> Self {
        val.into()
    }
}

impl From<SkinVertexExTuple> for Vertex {
    fn from(val: SkinVertexExTuple) -> Self {
        val.into()
    }
}

/// The "easy" vertex. If you are not sure what you want, this is probably it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct NormalVertex {
    /// POSITION
    pub position: Vec3,

    /// TEXCOORD0
    pub tex_coord: Vec2,
}

impl NormalVertex {
    fn is_finite(self) -> bool {
        let position = vec3_is_finite(self.position) as u8;
        let tex_coord = vec2_is_finite(self.tex_coord) as u8;

        position * tex_coord == 1
    }
}

impl Eq for NormalVertex {}

impl From<NormalVertexArray> for NormalVertex {
    fn from(val: NormalVertexArray) -> Self {
        Self {
            position: vec3(val[0], val[1], val[2]),
            tex_coord: vec2(val[3], val[4]),
        }
    }
}

impl From<NormalVertexArrays> for NormalVertex {
    fn from((position, tex_coord): NormalVertexArrays) -> Self {
        Self {
            position: vec3(position[0], position[1], position[2]),
            tex_coord: vec2(tex_coord[0], tex_coord[1]),
        }
    }
}

impl From<NormalVertexTuple> for NormalVertex {
    fn from((position, tex_coord): NormalVertexTuple) -> Self {
        Self {
            position,
            tex_coord,
        }
    }
}

impl Ord for NormalVertex {
    fn cmp(&self, other: &Self) -> Ordering {
        let res = self
            .position
            .partial_cmp(&other.position)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        self.tex_coord
            .partial_cmp(&other.tex_coord)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for NormalVertex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Same as `Normal`, except that the additional vertex attributes (normal and tanget) are
/// specified here as opposed to being calculated before use.
///
/// Only needed if you want more control over the normal/tangent generation process.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct NormalVertexEx {
    /// POSITION
    pub position: Vec3,

    /// NORMAL
    pub normal: Vec3,

    /// TANGENT (four component with handedness)
    pub tangent: Vec4,

    /// TEXCOORD0
    pub tex_coord: Vec2,
}

impl NormalVertexEx {
    fn is_finite(self) -> bool {
        let position = vec3_is_finite(self.position) as u8;
        let normal = vec3_is_finite(self.normal) as u8;
        let tangent = vec4_is_finite(self.tangent) as u8;
        let tex_coord = vec2_is_finite(self.tex_coord) as u8;

        position * normal * tangent * tex_coord == 1
    }
}

impl Eq for NormalVertexEx {}

impl From<NormalVertexExArray> for NormalVertexEx {
    fn from(val: NormalVertexExArray) -> Self {
        Self {
            position: vec3(val[0], val[1], val[2]),
            normal: vec3(val[3], val[4], val[5]),
            tangent: vec4(val[6], val[7], val[8], val[9]),
            tex_coord: vec2(val[10], val[11]),
        }
    }
}

impl From<NormalVertexExArrays> for NormalVertexEx {
    fn from((position, normal, tangent, tex_coord): NormalVertexExArrays) -> Self {
        Self {
            position: vec3(position[0], position[1], position[2]),
            normal: vec3(normal[0], normal[1], normal[2]),
            tangent: vec4(tangent[0], tangent[1], tangent[2], tangent[3]),
            tex_coord: vec2(tex_coord[0], tex_coord[1]),
        }
    }
}

impl From<NormalVertexExTuple> for NormalVertexEx {
    fn from((position, normal, tangent, tex_coord): NormalVertexExTuple) -> Self {
        Self {
            position,
            normal,
            tangent,
            tex_coord,
        }
    }
}

impl Ord for NormalVertexEx {
    fn cmp(&self, other: &Self) -> Ordering {
        let res = self
            .position
            .partial_cmp(&other.position)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .normal
            .partial_cmp(&other.normal)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .tangent
            .partial_cmp(&other.tangent)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        self.tex_coord
            .partial_cmp(&other.tex_coord)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for NormalVertexEx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Same as `Normal`, except that additional vertex attributes for joins and weights are added.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct SkinVertex {
    /// POSITION
    pub position: Vec3,

    /// JOINTS (four channel mix)
    pub joints: Vec4,

    /// WEIGHTS (four channel mix)
    pub weights: Vec4,

    /// TEXCOORD0
    pub tex_coord: Vec2,
}

impl SkinVertex {
    fn is_finite(self) -> bool {
        let position = vec3_is_finite(self.position) as u8;
        let joints = vec4_is_finite(self.joints) as u8;
        let weights = vec4_is_finite(self.weights) as u8;
        let tex_coord = vec2_is_finite(self.tex_coord) as u8;

        position * joints * weights * tex_coord == 1
    }
}

impl Eq for SkinVertex {}

impl From<SkinVertexArray> for SkinVertex {
    fn from(val: SkinVertexArray) -> Self {
        Self {
            position: vec3(val[0], val[1], val[2]),
            joints: vec4(val[5], val[6], val[7], val[8]),
            weights: vec4(val[9], val[10], val[11], val[12]),
            tex_coord: vec2(val[3], val[4]),
        }
    }
}

impl From<SkinVertexArrays> for SkinVertex {
    fn from((position, joints, weights, tex_coord): SkinVertexArrays) -> Self {
        Self {
            position: vec3(position[0], position[1], position[2]),
            joints: vec4(joints[0], joints[1], joints[2], joints[3]),
            weights: vec4(weights[0], weights[1], weights[2], weights[3]),
            tex_coord: vec2(tex_coord[0], tex_coord[1]),
        }
    }
}

impl From<(Vec3, Vec4, Vec4, Vec2)> for SkinVertex {
    fn from((position, joints, weights, tex_coord): (Vec3, Vec4, Vec4, Vec2)) -> Self {
        Self {
            position,
            joints,
            weights,
            tex_coord,
        }
    }
}

impl Ord for SkinVertex {
    fn cmp(&self, other: &Self) -> Ordering {
        let res = self
            .position
            .partial_cmp(&other.position)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .joints
            .partial_cmp(&other.joints)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .weights
            .partial_cmp(&other.weights)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        self.tex_coord
            .partial_cmp(&other.tex_coord)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SkinVertex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Like `NormalEx`, a combination of `Skin` and extra normal and tangent vertex attributes.
///
/// Only needed if you want more control over the normal/tangent generation process.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct SkinVertexEx {
    /// POSITION
    pub position: Vec3,

    /// NORMAL
    pub normal: Vec3,

    /// TANGENT (four component with handedness)
    pub tangent: Vec4,

    /// JOINTS (four channel mix)
    pub joints: Vec4,

    /// WEIGHTS (four channel mix)
    pub weights: Vec4,

    /// TEXCOORD0
    pub tex_coord: Vec2,
}

impl SkinVertexEx {
    fn is_finite(self) -> bool {
        let position = vec3_is_finite(self.position) as u8;
        let normal = vec3_is_finite(self.normal) as u8;
        let tangent = vec4_is_finite(self.tangent) as u8;
        let joints = vec4_is_finite(self.joints) as u8;
        let weights = vec4_is_finite(self.weights) as u8;
        let tex_coord = vec2_is_finite(self.tex_coord) as u8;

        position * normal * tangent * joints * weights * tex_coord == 1
    }
}

impl Eq for SkinVertexEx {}

impl From<SkinVertexExArray> for SkinVertexEx {
    fn from(val: SkinVertexExArray) -> Self {
        Self {
            position: vec3(val[0], val[1], val[2]),
            normal: vec3(val[3], val[4], val[5]),
            tangent: vec4(val[6], val[7], val[8], val[9]),
            joints: vec4(val[10], val[11], val[12], val[13]),
            weights: vec4(val[14], val[15], val[16], val[17]),
            tex_coord: vec2(val[18], val[19]),
        }
    }
}

impl From<SkinVertexExArrays> for SkinVertexEx {
    fn from((position, normal, tangent, joints, weights, tex_coord): SkinVertexExArrays) -> Self {
        Self {
            position: vec3(position[0], position[1], position[2]),
            normal: vec3(normal[0], normal[1], normal[2]),
            tangent: vec4(tangent[0], tangent[1], tangent[2], tangent[3]),
            joints: vec4(joints[0], joints[1], joints[2], joints[3]),
            weights: vec4(weights[0], weights[1], weights[2], weights[3]),
            tex_coord: vec2(tex_coord[0], tex_coord[1]),
        }
    }
}

impl From<(Vec3, Vec3, Vec4, Vec4, Vec4, Vec2)> for SkinVertexEx {
    fn from(
        (position, normal, tangent, joints, weights, tex_coord): (
            Vec3,
            Vec3,
            Vec4,
            Vec4,
            Vec4,
            Vec2,
        ),
    ) -> Self {
        Self {
            position,
            normal,
            tangent,
            joints,
            weights,
            tex_coord,
        }
    }
}

impl Ord for SkinVertexEx {
    fn cmp(&self, other: &Self) -> Ordering {
        let res = self
            .position
            .partial_cmp(&other.position)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .normal
            .partial_cmp(&other.normal)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .tangent
            .partial_cmp(&other.tangent)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .joints
            .partial_cmp(&other.joints)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        let res = self
            .weights
            .partial_cmp(&other.weights)
            .unwrap_or(Ordering::Equal);
        if res != Ordering::Less {
            return res;
        }

        self.tex_coord
            .partial_cmp(&other.tex_coord)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SkinVertexEx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
