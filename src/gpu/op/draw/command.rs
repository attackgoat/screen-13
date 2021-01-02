use {
    super::{LineVertex, Material},
    crate::{
        color::{AlphaColor, Color},
        gpu::{MeshFilter, ModelRef, Pose},
        math::{vec3_is_finite, CoordF, Mat4, Sphere, Vec3},
    },
    std::{marker::PhantomData, num::FpCategory, ops::Range},
};

pub type PointLightIter<'a> = CommandIter<'a, PointLightCommand>;
pub type RectLightIter<'a> = CommandIter<'a, RectLightCommand>;
pub type SpotlightIter<'a> = CommandIter<'a, SpotlightCommand>;
pub type SunlightIter<'a> = CommandIter<'a, SunlightCommand>;

// TODO: Voxels, landscapes, water, god rays, particle systems
/// An expressive type which allows specification of individual drawing operations.
pub enum Command {
    /// Draws a line segment.
    Line(LineCommand),

    /// Draws a model.
    Model(ModelCommand),

    /// Draws a point light.
    PointLight(PointLightCommand),

    /// Draws a rectangular light.
    RectLight(RectLightCommand),

    /// Draws a spotlight.
    Spotlight(SpotlightCommand),

    /// Draws sunlight.
    Sunlight(SunlightCommand),
}

impl Command {
    pub(crate) fn as_line(&self) -> Option<&LineCommand> {
        match self {
            Self::Line(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_model(&self) -> Option<&ModelCommand> {
        match self {
            Self::Model(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_point_light(&self) -> Option<&PointLightCommand> {
        match self {
            Self::PointLight(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_rect_light(&self) -> Option<&RectLightCommand> {
        match self {
            Self::RectLight(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_spotlight(&self) -> Option<&SpotlightCommand> {
        match self {
            Self::Spotlight(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_sunlight(&self) -> Option<&SunlightCommand> {
        match self {
            Self::Sunlight(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn is_line(&self) -> bool {
        self.as_line().is_some()
    }

    pub(crate) fn is_model(&self) -> bool {
        self.as_model().is_some()
    }

    pub(crate) fn is_point_light(&self) -> bool {
        self.as_point_light().is_some()
    }

    pub(crate) fn is_rect_light(&self) -> bool {
        self.as_rect_light().is_some()
    }

    pub(crate) fn is_spotlight(&self) -> bool {
        self.as_spotlight().is_some()
    }

    pub(crate) fn is_sunlight(&self) -> bool {
        self.as_sunlight().is_some()
    }

    /// Draws a line between the given coordinates using a constant width and two colors. The colors
    /// specify a gradient if
    /// they differ. Generally intended to support debugging use cases such as drawing bounding
    /// boxes.
    pub fn line<S: Into<Vec3>, SC: Into<AlphaColor>, E: Into<Vec3>, EC: Into<AlphaColor>>(
        start: S,
        start_color: SC,
        end: E,
        end_color: EC,
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
        })
    }

    /// Draws a model using the given material and world transform.
    pub fn model<M: Into<Mesh>>(mesh: M, material: Material, transform: Mat4) -> Self {
        let mesh = mesh.into();
        Self::Model(ModelCommand {
            camera_order: f32::NAN,
            material,
            mesh_filter: mesh.filter,
            model: mesh.model,
            pose: mesh.pose,
            transform,
        })
    }

    /// Draws a point light using the given values.
    pub fn point_light(center: Vec3, color: Color, lumens: f32, radius: f32) -> Self {
        Self::PointLight(PointLightCommand {
            center,
            color,
            lumens,
            radius,
        })
    }

    /// Draws a spotlight with the given color, position/orientation, and shape.
    ///
    /// _Note_: Spotlights have a hemispherical cap on the bottom, so a given `range` will be the
    /// maximum range and you may not see any light on objects at that distance. Move the light a
    /// bit towards the object to enter the penumbra.
    ///
    /// # Arguments
    ///
    /// * `color` - Full-bright color of the cone area.
    /// * `power` - sRGB power value normalized for gamma, can be greater than 1.0.
    /// * `pos` - Position of the pointy end of the spotlight.
    /// * `normal` - Orientation of the spotlight from `pos` towards the spotlight target.
    /// * `range` - Define the full-bright section along `normal`.
    /// * `cone_angle` - Interior angle of the full-bright portion of the spotlight, in degrees.
    /// * `penumbra_angle` - Additional angle which fades from `color` to tranparent, in degrees.
    pub fn spotlight(
        color: Color,
        lumens: f32,
        position: Vec3,
        normal: Vec3,
        range: Range<f32>,
        cone_angle: f32,
        penumbra_angle: f32,
    ) -> Self {
        assert_eq!(lumens.classify(), FpCategory::Normal);
        assert!(vec3_is_finite(position));
        assert!(vec3_is_finite(normal));
        assert_eq!(range.start.classify(), FpCategory::Normal);
        assert_eq!(range.end.classify(), FpCategory::Normal);
        assert!(range.start < range.end);
        assert_eq!(cone_angle.classify(), FpCategory::Normal);
        assert!(penumbra_angle >= 0.0);
        assert!(cone_angle + penumbra_angle < 180.0);

        let half = cone_angle * 0.5;
        let cone_radius = half.tan() * range.end;
        let penumbra_radius = (half + penumbra_angle).tan() * range.end;
        let top_radius = 0.0; // TODO!

        Self::Spotlight(SpotlightCommand {
            color,
            cone_radius,
            lumens,
            normal,
            penumbra_radius,
            position,
            range,
            top_radius,
        })
    }
}

pub struct CommandIter<'a, T> {
    __: PhantomData<T>,
    cmds: &'a [Command],
    idx: usize,
}

impl<'a, T> CommandIter<'a, T> {
    pub fn new(cmds: &'a [Command]) -> Self {
        Self {
            __: PhantomData,
            cmds,
            idx: 0,
        }
    }
}

impl<'a> Iterator for CommandIter<'a, PointLightCommand> {
    type Item = &'a PointLightCommand;

    fn next(&mut self) -> Option<Self::Item> {
        self.cmds
            .get(self.idx)
            .map(|cmd| {
                self.idx += 1;
                cmd.as_point_light()
            })
            .unwrap_or_default()
    }
}

impl<'a> Iterator for CommandIter<'a, RectLightCommand> {
    type Item = &'a RectLightCommand;

    fn next(&mut self) -> Option<Self::Item> {
        self.cmds
            .get(self.idx)
            .map(|cmd| {
                self.idx += 1;
                cmd.as_rect_light()
            })
            .unwrap_or_default()
    }
}

impl<'a> Iterator for CommandIter<'a, SpotlightCommand> {
    type Item = &'a SpotlightCommand;

    fn next(&mut self) -> Option<Self::Item> {
        self.cmds
            .get(self.idx)
            .map(|cmd| {
                self.idx += 1;
                cmd.as_spotlight()
            })
            .unwrap_or_default()
    }
}

impl<'a> Iterator for CommandIter<'a, SunlightCommand> {
    type Item = &'a SunlightCommand;

    fn next(&mut self) -> Option<Self::Item> {
        self.cmds
            .get(self.idx)
            .map(|cmd| {
                self.idx += 1;
                cmd.as_sunlight()
            })
            .unwrap_or_default()
    }
}

// TODO: This is crufty, fix.
/// Description of a single line segment.
#[derive(Clone, Debug)]
pub struct LineCommand {
    /// The start and end vertices to draw.
    pub vertices: [LineVertex; 2],
}

/// Container of a shared `Model` reference and the optional filter or pose that might be used
/// during rendering.
#[derive(Debug)]
pub struct Mesh {
    /// The mesh or meshes, if set, to render from within the larger collection of meshes this model
    /// may contain.
    ///
    /// `MeshFilter` instances are acquired from the `Model::filter(&self, ...)` function and can
    /// only be used with the same model they are retrieved from.
    pub filter: Option<MeshFilter>,

    /// The shared model reference to render.
    pub model: ModelRef,

    /// The animation pose, if set, to use while rendering this model.
    pub pose: Option<Pose>,
}

impl From<ModelRef> for Mesh {
    fn from(model: ModelRef) -> Self {
        Self {
            filter: None,
            model,
            pose: None,
        }
    }
}

impl From<(ModelRef, MeshFilter)> for Mesh {
    fn from((model, filter): (ModelRef, MeshFilter)) -> Self {
        Self {
            filter: Some(filter),
            model,
            pose: None,
        }
    }
}

impl From<(ModelRef, Option<MeshFilter>)> for Mesh {
    fn from((model, filter): (ModelRef, Option<MeshFilter>)) -> Self {
        Self {
            filter,
            model,
            pose: None,
        }
    }
}

impl From<(ModelRef, Pose)> for Mesh {
    fn from((model, pose): (ModelRef, Pose)) -> Self {
        Self {
            filter: None,
            model,
            pose: Some(pose),
        }
    }
}

impl From<(ModelRef, Option<Pose>)> for Mesh {
    fn from((model, pose): (ModelRef, Option<Pose>)) -> Self {
        Self {
            filter: None,
            model,
            pose,
        }
    }
}

impl From<(ModelRef, MeshFilter, Pose)> for Mesh {
    fn from((model, filter, pose): (ModelRef, MeshFilter, Pose)) -> Self {
        Self {
            filter: Some(filter),
            model,
            pose: Some(pose),
        }
    }
}

impl From<(ModelRef, Option<MeshFilter>, Option<Pose>)> for Mesh {
    fn from((model, filter, pose): (ModelRef, Option<MeshFilter>, Option<Pose>)) -> Self {
        Self {
            filter,
            model,
            pose,
        }
    }
}

/// Description of a model, which may be posed or filtered.
#[derive(Debug)]
pub struct ModelCommand {
    // TODO: Could probably be u16?
    pub(super) camera_order: f32,

    /// The material to use while rendering this model.
    pub material: Material,

    /// The mesh or meshes, if set, to render from within the larger collection of meshes this model
    /// may contain.
    ///
    /// `MeshFilter` instances are acquired from the `Model::filter(&self, ...)` function and can
    /// only be used with the same model they are retrieved from.
    pub mesh_filter: Option<MeshFilter>,

    /// The shared model reference to render.
    pub model: ModelRef,

    /// The animation pose, if set, to use while rendering this model.
    pub pose: Option<Pose>,

    /// The generalized matrix transform usually used to desctibe translation, scale and rotation.
    ///
    /// _NOTE_: This is the "world" matrix; view and projection matrices are handled automatically.
    pub transform: Mat4,
}

/// Description of a point light shining on models.
#[derive(Clone, Debug)]
pub struct PointLightCommand {
    /// The location of the center of this light in world space.
    pub center: Vec3,

    /// Color of the projected light.
    pub color: Color,

    /// Scalar light "power" value, modelled afterm lumens but not realistically.
    pub lumens: f32,

    /// Distance from `center` to the furthest reach of the point light.
    pub radius: f32,
}

/// Description of a rectangular light shining on models.
///
/// The lit area forms a [square frustum](https://en.wikipedia.org/wiki/Frustum).
#[derive(Clone, Debug)]
pub struct RectLightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// The width and height of the rectangular light, when viewed from above (the unlit side).
    pub dims: CoordF,

    /// Scalar light "power" value, modelled afterm lumens but not realistically.
    pub lumens: f32,

    /// Direction of the light rays.
    pub normal: Vec3,

    /// Top-left corner, when viewed from above (the unlit side).
    pub position: Vec3,

    /// size of the penumbra area beyond the box formed by `position` and `range` which fades from
    /// `color` to transparent.
    pub radius: f32,

    /// Distance from `position` to the bottom of the rectangular light.
    pub range: f32,
}

impl RectLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this rectangular light, including
    /// the penumbra
    pub(self) fn bounds(&self) -> Sphere {
        todo!();
    }
}

/// Description of a spotlight shining on models.
#[derive(Clone, Debug)]
pub struct SpotlightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// radius of the spotlight cone from the center to the edge.
    pub cone_radius: f32,

    /// Scalar light "power" value, modelled afterm lumens but not realistically.
    pub lumens: f32,

    /// Direction of the light rays.
    pub normal: Vec3,

    /// Additional radius beyond `cone_radius` which fades from `color` to transparent.
    pub penumbra_radius: f32,

    /// Position of the pointy end.
    pub position: Vec3,

    /// Lit distance from `position` and to the bottom of the spotlight.
    pub range: Range<f32>,

    /// TODO: Refactor
    pub top_radius: f32,
}

/// Description of sunlight shining on models.
#[derive(Clone, Debug)]
pub struct SunlightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// Scalar light "power" value, modelled afterm lumens but not realistically.
    pub lumens: f32,

    /// Direction of the light rays.
    pub normal: Vec3,
}
