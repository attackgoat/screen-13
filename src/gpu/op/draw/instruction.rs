use {
    super::MeshDrawInstruction,
    crate::gpu::{data::CopyRange, Data},
    std::ops::Range,
};

// Commands specified by the client become Instructions used by `DrawOp`
pub enum Instruction<'a> {
    CopyVertices((&'a mut Data, &'a [CopyRange])),
    //Light(LightInstruction),
    DrawLines((&'a mut Data, u32)),
    //DrawPointLights(&),
    DrawRectLightBegin(&'a mut Data),
    DrawRectLight(),
    DrawRectLightEnd,
    //Mesh(MeshInstruction<'a>),
    // Spotlight(SpotlightCommand),
    // Sunlight(SunlightCommand),
    // Transparency((f32, MeshCommand<'a>)),
    TransferData((&'a mut Data, &'a mut Data)),
    WriteVertces((&'a mut Data, Range<u64>)),
}

impl Instruction<'_> {
    // pub fn as_light(&self) -> Option<&LightInstruction> {
    //     match self {
    //         Self::Light(instr) => Some(instr),
    //         _ => None,
    //     }
    // }

    // pub fn as_line(&self) -> Option<&LineInstruction> {
    //     match self {
    //         Self::Line(instr) => Some(instr),
    //         _ => None,
    //     }
    // }

    // pub fn as_mesh(&self) -> Option<&MeshInstruction> {
    //     match self {
    //         Self::Mesh(instr) => Some(instr),
    //         _ => None,
    //     }
    // }

    // pub fn as_sunlight(&self) -> Option<&SunlightCommand> {
    //     match self {
    //         Self::Sunlight(cmd) => Some(cmd),
    //         _ => None,
    //     }
    // }

    // pub fn is_light(&self) -> bool {
    //     match self {
    //         Self::Light(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn is_line(&self) -> bool {
    //     match self {
    //         Self::Line(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn is_mesh(&self) -> bool {
    //     match self {
    //         Self::Mesh(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn is_mesh_single(&self) -> bool {
    //     match self {
    //         Self::Transparency(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn is_mesh_trans(&self) -> bool {
    //     match self {
    //         Self::Transparency(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn is_spotlight(&self) -> bool {
    //     match self {
    //         Self::Spotlight(_) => true,
    //         _ => false,
    //     }
    // }

    // // pub fn is_stop(&self) -> bool {
    // //     match self {
    // //         Self::Stop => true,
    // //         _ => false,
    // //     }
    // // }

    // pub fn is_sunlight(&self) -> bool {
    //     match self {
    //         Self::Sunlight(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn into_line(self) -> Option<LineInstruction> {
    //     match self {
    //         Self::Line(instr) => Some(instr),
    //         _ => None,
    //     }
    // }

    // pub fn into_mesh(self) -> Option<MeshCommand<'a>> {
    //     match self {
    //         Self::Mesh((_, cmd))
    //         | Self::Transparency((_, cmd)) => Some(cmd),
    //         _ => None,
    //     }
    // }
}

// pub enum LineInstruction<'i> {
//     Draw(DrawLineInstruction<'i>),
//     SetWidth(f32),
// }

// pub struct DrawLineInstruction<'i> {
//     pub data: &'i [u8],
//     pub width: f32,
// }

// impl DrawLineInstruction<'_> {
//     pub fn vertices(&self) -> u32 {
//         (self.data.len() / LINE_VERTEX_LEN) as _
//     }
// }

pub enum MeshInstruction<'i> {
    BindDescriptorSet(usize),
    Draw(MeshDrawInstruction<'i>),
}
