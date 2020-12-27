use {crate::math::{Vec3,Vec2, Mat3, Mat4}, gfx_hal::pso::ShaderStageFlags, std::ops::Range};

pub type ShaderRange = (ShaderStageFlags, Range<u32>);

// General-use consts and types (single values only)

pub const VERTEX_MAT4: [ShaderRange; 1] = [(ShaderStageFlags::VERTEX, 0..64)];

#[repr(C)]
pub struct Mat4PushConst(pub Mat4);

impl AsRef<[u32; 16]> for Mat4PushConst {
    #[inline]
    fn as_ref(&self) -> &[u32; 16] {
        unsafe { &*(self as *const Self as *const [u32; 16]) }
    }
}

#[repr(C)]
pub struct U32PushConst(pub u32);

impl AsRef<[u32; 1]> for U32PushConst {
    #[inline]
    fn as_ref(&self) -> &[u32; 1] {
        unsafe { &*(self as *const Self as *const [u32; 1]) }
    }
}

// Specific-use consts and types (gives context to fields and alignment control)

pub const BLEND: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 64..72),
];
pub const CALC_VERTEX_ATTRS: [ShaderRange; 1] = [(ShaderStageFlags::COMPUTE, 0..8)];
pub const DECODE_RGB_RGBA: [ShaderRange; 1] = [(ShaderStageFlags::COMPUTE, 0..4)];
pub const DRAW_POINT_LIGHT: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 0..0),
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
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 64..80),
];
pub const FONT_OUTLINE: [ShaderRange; 2] = [
    (ShaderStageFlags::VERTEX, 0..64),
    (ShaderStageFlags::FRAGMENT, 64..96),
];
pub const SKYDOME: [ShaderRange; 0] = [];
pub const TEXTURE: [ShaderRange; 1] = [(ShaderStageFlags::VERTEX, 0..80)];

#[repr(C)]
pub struct CalcVertexAttrsPushConsts {
    pub base_idx: u32,
    pub base_vertex: u32,
}

impl AsRef<[u32; 2]> for CalcVertexAttrsPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 2] {
        unsafe { &*(self as *const Self as *const [u32; 2]) }
    }
}

#[repr(C)]
pub struct PointLightPushConsts {
    pub intensity: Vec3,
    pub radius: f32,
}

impl AsRef<[u32; 4]> for PointLightPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 4] {
        unsafe { &*(self as *const Self as *const [u32; 4]) }
    }
}

#[repr(C)]
pub struct RectLightPushConsts {
    pub dims: Vec2,
    pub intensity: Vec3,
    pub normal: Vec3,
    pub position: Vec3,
    pub radius: f32,
    pub range: f32,
    pub view_proj: Mat4,
}

impl AsRef<[u32; 6]> for RectLightPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}

#[repr(C)]
pub struct SkydomeFragmentPushConsts {
    pub time: f32,
    pub weather: f32,
}

impl AsRef<[u32; 2]> for SkydomeFragmentPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 2] {
        unsafe { &*(self as *const Self as *const [u32; 2]) }
    }
}

#[repr(C)]
pub struct SkydomeVertexPushConsts {
    pub star_rotation: Mat3,
    pub sun_normal: Vec3,
    pub view_proj: Mat4,
}

impl AsRef<[u32; 37]> for SkydomeVertexPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 37] {
        unsafe { &*(self as *const Self as *const [u32; 37]) }
    }
}

#[repr(C)]
pub struct SunlightPushConsts {
    pub intensity: Vec3,
    pub normal: Vec3,
}

impl AsRef<[u32; 6]> for SunlightPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}

#[repr(C)]
pub struct SpotlightPushConsts {
    pub intensity: Vec3,
    pub normal: Vec3,
}

impl AsRef<[u32; 6]> for SpotlightPushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 6] {
        unsafe { &*(self as *const Self as *const [u32; 6]) }
    }
}

#[repr(C)]
pub struct WritePushConsts {
    pub offset: Vec2,
    pub scale: Vec2,
    pub transform: Mat4,
}

impl AsRef<[u32; 20]> for WritePushConsts {
    #[inline]
    fn as_ref(&self) -> &[u32; 20] {
        unsafe { &*(self as *const Self as *const [u32; 20]) }
    }
}
