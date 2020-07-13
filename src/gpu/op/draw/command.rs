use {
    super::{Material, SpotlightCommand, SunlightCommand},
    crate::{
        color::AlphaColor,
        gpu::Mesh,
        math::{Mat4, Vec3},
    },
};

#[derive(Debug)]
pub enum Command<'a> {
    Line(LineCommand),
    Mesh(MeshCommand<'a>),
    Spotlight(SpotlightCommand),
    Sunlight(SunlightCommand),
}

// TODO: This file defines three ways of writing new commands: from a Command function, from a tuple, or from the structure funtions. Do we need all these?

impl<'a> Command<'a> {
    pub fn line<S: Into<Vec3>, SC: Into<AlphaColor>, E: Into<Vec3>, EC: Into<AlphaColor>>(
        start: S,
        start_color: SC,
        end: E,
        end_color: EC,
        width: f32,
    ) -> Self {
        Self::Line(LineCommand {
            vertices: [
                LineVertex {
                    color: start_color.into(),
                    pos: start.into(),
                },
                LineVertex {
                    color: end_color.into(),
                    pos: end.into(),
                },
            ],
            width,
        })
    }
}

impl<'a> From<(Vec3, AlphaColor, Vec3, AlphaColor, f32)> for Command<'a> {
    fn from(
        (start, start_color, end, end_color, width): (Vec3, AlphaColor, Vec3, AlphaColor, f32),
    ) -> Self {
        Self::Line(LineCommand {
            vertices: [
                LineVertex {
                    color: start_color,
                    pos: start,
                },
                LineVertex {
                    color: end_color,
                    pos: end,
                },
            ],
            width,
        })
    }
}

impl<'a> From<(&'a Mesh, Mat4)> for Command<'a> {
    fn from((mesh, transform): (&'a Mesh, Mat4)) -> Self {
        (mesh, transform, Material::default()).into()
    }
}

impl<'a> From<(&'a Mesh, Mat4, Material)> for Command<'a> {
    fn from((mesh, transform, material): (&'a Mesh, Mat4, Material)) -> Self {
        Self::Mesh(MeshCommand {
            material,
            mesh,
            transform,
        })
    }
}

#[derive(Debug)]
pub struct LineCommand {
    pub vertices: [LineVertex; 2],
    pub width: f32,
}

impl LineCommand {
    pub fn new(vertices: [LineVertex; 2]) -> Self {
        Self::new_width(vertices, 1.0)
    }

    pub fn new_width(vertices: [LineVertex; 2], width: f32) -> Self {
        Self { vertices, width }
    }

    // TODO: pub fn new_perspective(start: &LineVertex, end: &LineVertex, camera: &impl Camera, width: f32, epsilon: f32) -> impl Iterator<LineVertex> {
    //     todo!()
    // }
}

#[derive(Debug)]
pub struct LineVertex {
    pub color: AlphaColor,
    pub pos: Vec3,
}

#[derive(Clone, Debug)]
pub struct MeshCommand<'a> {
    pub material: Material,
    pub mesh: &'a Mesh,
    pub transform: Mat4,
}
