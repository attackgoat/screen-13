use {
    super::{Material, SpotlightCommand, SunlightCommand},
    crate::{
        color::AlphaColor,
        gpu::{Bitmap, Mesh},
        math::{Mat4, Rect, RectF, Vec3},
    },
};

#[derive(Debug)]
pub struct BitmapCommand<'a> {
    pub bitmap: &'a Bitmap,
    /// The floating-point area to draw the bitmap
    pub dst: RectF,
    /// The fixed-point area of the bitmap to draw
    pub src: Rect,
    /// Values greater than zero draw above lines and meshes while values less than or equal to zero draw below. Bitmaps are sorted
    /// relative to each other as well.
    pub z: isize,
}

#[derive(Debug)]
pub enum Command<'a> {
    Bitmap(BitmapCommand<'a>),
    Line(LineCommand),
    Mesh(MeshCommand<'a>),
    Spotlight(SpotlightCommand),
    Sunlight(SunlightCommand),
}

impl<'a> From<(&'a Bitmap, Rect, RectF, isize)> for Command<'a> {
    fn from((bitmap, src, dst, z): (&'a Bitmap, Rect, RectF, isize)) -> Self {
        Self::Bitmap(BitmapCommand {
            bitmap, dst, src, z,
        })
    }
}

// TODO: I dislike these 'from tuple' things, maybe just a bunch of easy-to-understand functions which create the commands would be nicer? 
impl<'a> From<(f32, Vec3, AlphaColor, Vec3, AlphaColor)> for Command<'a> {
    fn from(
        (width, start, start_color, end, end_color): (f32, Vec3, AlphaColor, Vec3, AlphaColor),
    ) -> Self {
        Self::Line(LineCommand {
            vertices: [
                LineVertex {
                    color: start_color,
                    position: start,
                },
                LineVertex {
                    color: end_color,
                    position: end,
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
    pub position: Vec3,
}

#[derive(Clone, Debug)]
pub struct MeshCommand<'a> {
    pub material: Material,
    pub mesh: &'a Mesh,
    pub transform: Mat4,
}
