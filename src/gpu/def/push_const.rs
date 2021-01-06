use {
    crate::math::{Mat4, Vec2, Vec3, Vec4},
    gfx_hal::pso::ShaderStageFlags,
    std::ops::Range,
};

/// The push constant structs created by this macro implement Default and provide a reference to the
/// underlying c-formatted data as a u32 slice. This makes it easy to use from our point of view and
/// it provides what GFX-HAL wants during command recording and submission. To align fields properly
/// you may need to insert private fields of the needed size.
///
/// Syntax and usage:
/// push_const_struct!(STRUCT_NAME {
///     [VISIBILITY_SPECIFIER] FIELD_NAME: FIELD_TYPE,
///     ...
/// });
macro_rules! push_const_struct {
    ($struct: ident { $($vis: vis $element: ident: $ty: ty,) * }) => {
        #[derive(Default)]
        #[repr(C)]
        pub struct $struct { $($vis $element: $ty),* }

        impl $struct {
            pub const BYTE_LEN: u32 = std::mem::size_of::<$struct>() as _;
            pub const U32_LEN: usize = (Self::BYTE_LEN >> 2) as _;

            // TODO: Have a ctor that only fills in the public fields?
            // pub fn new($($element: $ty),*) {
            // }
        }

        impl AsRef<[u32; Self::U32_LEN]> for $struct {
            #[inline]
            fn as_ref(&self) -> &[u32; Self::U32_LEN] {
                unsafe { &*(self as *const Self as *const [u32; Self::U32_LEN]) }
            }
        }
    }
}

pub type ShaderRange = (ShaderStageFlags, Range<u32>);

// General-use consts and types (single values only)

pub const VERTEX_MAT4: [ShaderRange; 1] = [(ShaderStageFlags::VERTEX, 0..64)];

push_const_struct!(Mat4PushConst {
    pub val: Mat4,
});
push_const_struct!(U32PushConst {
    pub val: u32,
});
push_const_struct!(Vec4PushConst {
    pub val: Vec4,
});

// Specific-use consts and types (gives context to fields and alignment control)

pub const BLEND: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 64..72),
];
pub const CALC_VERTEX_ATTRS: [ShaderRange; 1] = [(ShaderStageFlags::COMPUTE, 0..8)];
pub const DECODE_RGB_RGBA: [ShaderRange; 1] = [(ShaderStageFlags::COMPUTE, 0..4)];
pub const DRAW_POINT_LIGHT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..Mat4PushConst::BYTE_LEN),
    (
        ShaderStageFlags::FRAGMENT,
        Mat4PushConst::BYTE_LEN..PointLightPushConsts::BYTE_LEN,
    ),
];
pub const DRAW_RECT_LIGHT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 0..0),
];
pub const DRAW_SPOTLIGHT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 0..0),
];
pub const DRAW_SUNLIGHT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 0..0),
];
pub const FONT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..Mat4PushConst::BYTE_LEN),
    (
        ShaderStageFlags::FRAGMENT,
        Mat4PushConst::BYTE_LEN..Mat4PushConst::BYTE_LEN + Vec4PushConst::BYTE_LEN,
    ),
];
pub const FONT_OUTLINE: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..Mat4PushConst::BYTE_LEN),
    (
        ShaderStageFlags::FRAGMENT,
        Mat4PushConst::BYTE_LEN..Mat4PushConst::BYTE_LEN + FontPushConsts::BYTE_LEN,
    ),
];
pub const SKYDOME: [ShaderRange; 2] = [
    (
        ShaderStageFlags::VERTEX,
        0..SkydomeVertexPushConsts::BYTE_LEN,
    ),
    (
        ShaderStageFlags::FRAGMENT,
        SkydomeVertexPushConsts::BYTE_LEN
            ..SkydomeVertexPushConsts::BYTE_LEN + SkydomeFragmentPushConsts::BYTE_LEN,
    ),
];
pub const TEXTURE: [ShaderRange; 1] = [(ShaderStageFlags::VERTEX, 0..80)];

push_const_struct!(CalcVertexAttrsPushConsts {
    pub base_vertex: u32,
    pub base_idx: u32,
});
push_const_struct!(FontPushConsts {
    pub glyph_color: Vec4,
    pub outline_color: Vec4,
});
// TODO: Could this be packed better?
push_const_struct!(PointLightPushConsts {
    pub view_proj_inv: Mat4,
    pub camera_eye: Vec3,
    _0: f32,
    pub depth_dims_inv: Vec2,
    _1: f32,
    _2: f32,
    pub light_center: Vec3,
    pub light_radius: f32,
    pub light_intensity: Vec3,
    _4: f32, // Not sure this is required
});
push_const_struct!(RectLightPushConsts {
    pub dims: Vec2,
    pub intensity: Vec3,
    pub normal: Vec3,
    pub position: Vec3,
    pub radius: f32,
    pub range: f32,
    pub view_proj: Mat4,
});
push_const_struct!(SkydomeFragmentPushConsts {
    pub sun_normal: Vec3,
    pub time: f32,
    _0: f32,
    pub weather: f32,
});
push_const_struct!(SkydomeVertexPushConsts {
    pub world_view_proj: Mat4,
    // `star_rotation` is a Mat3 in GLSL; but we have to break it up like this for alignment
    pub star_rotation_col0: Vec3,
    _0: f32,
    pub star_rotation_col1: Vec3,
    _1: f32,
    pub star_rotation_col2: Vec3,
    _2: f32, // This is required to keep padding with the following shader stage
});
push_const_struct!(SunlightPushConsts {
    pub intensity: Vec3,
    pub normal: Vec3,
});
push_const_struct!(SpotlightPushConsts {
    pub intensity: Vec3,
    pub normal: Vec3,
});
push_const_struct!(WriteVertexPushConsts {
    pub offset: Vec2,
    pub scale: Vec2,
    pub transform: Mat4,
});
push_const_struct!(WriteFragmentPushConsts {
    pub ab: f32,
    pub ab_inv: f32,
});
