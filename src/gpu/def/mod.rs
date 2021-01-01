pub mod compute;
pub mod desc_set_layout;
pub mod graphics;
pub mod push_const;
pub mod render_pass;
pub mod vertex;

pub use self::{compute::Compute, graphics::Graphics};

use {
    super::{BlendMode, MaskMode, MatteMode},
    crate::pak::IndexType,
    gfx_hal::{
        format::Format,
        pso::{BufferDescriptorFormat, BufferDescriptorType, DescriptorType, ImageDescriptorType},
    },
};

const READ_ONLY_BUF: DescriptorType = DescriptorType::Buffer {
    format: BufferDescriptorFormat::Structured {
        dynamic_offset: false,
    },
    ty: BufferDescriptorType::Storage { read_only: true },
};
const READ_ONLY_IMG: DescriptorType = DescriptorType::Image {
    ty: ImageDescriptorType::Sampled { with_sampler: true },
};
const READ_WRITE_BUF: DescriptorType = DescriptorType::Buffer {
    format: BufferDescriptorFormat::Structured {
        dynamic_offset: false,
    },
    ty: BufferDescriptorType::Storage { read_only: false },
};
const READ_WRITE_IMG: DescriptorType = DescriptorType::Image {
    ty: ImageDescriptorType::Storage { read_only: false },
};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CalcVertexAttrsComputeMode {
    pub idx_ty: IndexType,
    pub skin: bool,
}

impl CalcVertexAttrsComputeMode {
    pub const U16: Self = Self {
        idx_ty: IndexType::U16,
        skin: false,
    };
    pub const U16_SKIN: Self = Self {
        idx_ty: IndexType::U16,
        skin: true,
    };
    pub const U32: Self = Self {
        idx_ty: IndexType::U32,
        skin: false,
    };
    pub const U32_SKIN: Self = Self {
        idx_ty: IndexType::U32,
        skin: true,
    };
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ColorRenderPassMode {
    pub fmt: Format,
    pub preserve: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum ComputeMode {
    CalcVertexAttrs(CalcVertexAttrsComputeMode),
    DecodeRgbRgba,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct DrawRenderPassMode {
    pub depth: Format,
    pub geom_buf: Format,
    pub light: Format,
    pub output: Format,
    pub post_fx: bool,
    pub skydome: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum GraphicsMode {
    Blend(BlendMode),
    Font(bool),
    Gradient(bool),
    DrawLine,
    DrawMesh,
    DrawPointLight,
    DrawRectLight,
    DrawSpotlight,
    DrawSunlight,
    Mask(MaskMode),
    Matte(MatteMode),
    Skydome,
    Texture,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum RenderPassMode {
    Color(ColorRenderPassMode),
    Draw(DrawRenderPassMode),
}
