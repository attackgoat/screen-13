use {
    crate::{
        color::{AlphaColor, Color},
        gpu::{Bitmap, MeshFilter, Model, Pose},
        math::{vec3_is_finite, Cone, CoordF, Mat4, Sphere, Vec3},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{
        cmp::Ordering,
        fmt::{Debug, Error, Formatter},
        hash::{Hash, Hasher},
        iter::{once, Once},
        marker::PhantomData,
        num::FpCategory,
        ops::Range,
    },
};

pub type PointLightIter<'a, P> = CommandIter<'a, PointLightCommand, P>;
pub type RectLightIter<'a, P> = CommandIter<'a, RectLightCommand, P>;
pub type SpotlightIter<'a, P> = CommandIter<'a, SpotlightCommand, P>;
pub type SunlightIter<'a, P> = CommandIter<'a, SunlightCommand, P>;

// TODO: Voxels, landscapes, water, god rays, particle systems
/// An expressive type which allows specification of individual drawing operations.
#[non_exhaustive]
pub enum Command<P>
where
    P: 'static + SharedPointerKind,
{
    /// Draws a line segment.
    Line(LineCommand),

    /// Draws a model.
    Model(ModelCommand<P>),

    /// Draws a point light.
    PointLight(PointLightCommand),

    /// Draws a rectangular light.
    RectLight(RectLightCommand),

    /// Draws a spotlight.
    Spotlight(SpotlightCommand),

    /// Draws sunlight.
    Sunlight(SunlightCommand),
}

impl<P> Command<P>
where
    P: SharedPointerKind,
{
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
    pub fn model<M: Into<Mesh<P>>>(mesh: M, material: Material<P>, transform: Mat4) -> Self {
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

    /// Draws a spotlight with the given values.
    ///
    /// # Arguments
    ///
    /// * `color` - Color of the projected light.
    /// * `lumens` - Scalar light "power" value, modelled after lumens but not realistically.
    /// * `position` - Position of the light source.
    /// * `normal` - Direction of the light rays.
    /// * `range` - Lit distance from `position` and to the bottom of the spotlight.
    /// * `angle` - Outer angle of the lit portion of the spotlight, in degrees.
    ///
    ///             Must be greater than zero and less than 180.
    pub fn spotlight(
        color: Color,
        lumens: f32,
        position: Vec3,
        normal: Vec3,
        range: Range<f32>,
        angle: f32,
    ) -> Self {
        assert_eq!(lumens.classify(), FpCategory::Normal);
        assert!(vec3_is_finite(position));
        assert!(vec3_is_finite(normal));
        assert_eq!(range.start.classify(), FpCategory::Normal);
        assert_eq!(range.end.classify(), FpCategory::Normal);
        assert!(range.start < range.end);
        assert_eq!(angle.classify(), FpCategory::Normal);
        assert!(angle > 0.0);
        assert!(angle < 180.0);

        let tan_half = (angle * 0.5).tan();
        let radius = tan_half * range.end;
        let radius_start = tan_half * range.start;

        Self::Spotlight(SpotlightCommand {
            color,
            lumens,
            normal,
            position,
            radius,
            radius_start,
            range,
            ..Default::default()
        })
    }

    pub(crate) fn as_line(&self) -> Option<&LineCommand> {
        match self {
            Self::Line(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_model(&self) -> Option<&ModelCommand<P>> {
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

    pub(crate) fn as_rect_light_mut(&mut self) -> Option<&mut RectLightCommand> {
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

    pub(crate) fn as_spotlight_mut(&mut self) -> Option<&mut SpotlightCommand> {
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
}

impl<P> Clone for Command<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        match self {
            Self::Line(line) => Self::Line(line.clone()),
            Self::Model(model) => Self::Model(model.clone()),
            Self::PointLight(light) => Self::PointLight(light.clone()),
            Self::RectLight(light) => Self::RectLight(light.clone()),
            Self::Spotlight(light) => Self::Spotlight(light.clone()),
            Self::Sunlight(light) => Self::Sunlight(light.clone()),
        }
    }
}

impl<P> Debug for Command<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Comand")
    }
}

impl<P> IntoIterator for Command<P>
where
    P: SharedPointerKind,
{
    type Item = Command<P>;
    type IntoIter = Once<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        once(self)
    }
}

pub struct CommandIter<'a, T, P>
where
    P: 'static + SharedPointerKind,
{
    __: PhantomData<T>,
    cmds: &'a [Command<P>],
    idx: usize,
}

impl<'a, T, P> CommandIter<'a, T, P>
where
    P: SharedPointerKind,
{
    pub fn new(cmds: &'a [Command<P>]) -> Self {
        Self {
            __: PhantomData,
            cmds,
            idx: 0,
        }
    }
}

impl<'a, P> Iterator for CommandIter<'a, PointLightCommand, P>
where
    P: SharedPointerKind,
{
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

impl<'a, P> Iterator for CommandIter<'a, RectLightCommand, P>
where
    P: SharedPointerKind,
{
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

impl<'a, P> Iterator for CommandIter<'a, SpotlightCommand, P>
where
    P: SharedPointerKind,
{
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

impl<'a, P> Iterator for CommandIter<'a, SunlightCommand, P>
where
    P: SharedPointerKind,
{
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
#[non_exhaustive]
pub struct LineCommand {
    /// The start and end vertices to draw.
    pub vertices: [LineVertex; 2],
}

/// TODO: Move me to the vertices module?
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LineVertex {
    pub color: AlphaColor,
    pub pos: Vec3,
}

/// Defines a PBR material.
///
/// _NOTE:_ Temporary. I think this will soon become an enum with more options, reflectance probes,
/// shadow maps, lots more
pub struct Material<P>
where
    P: 'static + SharedPointerKind,
{
    /// Three channel base color, aka albedo or diffuse, of the material.
    pub color: Shared<Bitmap<P>, P>,

    /// A two channel bitmap of the metalness (red) and roughness (green) PBR parameters.
    pub metal_rough: Shared<Bitmap<P>, P>,

    /// A standard three channel normal map.
    pub normal: Shared<Bitmap<P>, P>,
}

impl<P> Clone for Material<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self {
            color: Shared::clone(&self.color),
            metal_rough: Shared::clone(&self.metal_rough),
            normal: Shared::clone(&self.normal),
        }
    }
}

impl<P> Debug for Material<P>
where
    P: SharedPointerKind,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Material")
    }
}

impl<P> Eq for Material<P> where P: SharedPointerKind {}

impl<P> Hash for Material<P>
where
    P: SharedPointerKind,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        Shared::as_ptr(&self.color).hash(state);
        Shared::as_ptr(&self.metal_rough).hash(state);
        Shared::as_ptr(&self.normal).hash(state);
    }
}

impl<P> Ord for Material<P>
where
    P: SharedPointerKind,
{
    fn cmp(&self, other: &Self) -> Ordering {
        let mut res = Shared::as_ptr(&self.color).cmp(&Shared::as_ptr(&other.color));
        if res != Ordering::Less {
            return res;
        }

        res = Shared::as_ptr(&self.metal_rough).cmp(&Shared::as_ptr(&other.metal_rough));
        if res != Ordering::Less {
            return res;
        }

        Shared::as_ptr(&self.normal).cmp(&Shared::as_ptr(&other.normal))
    }
}

impl<P> PartialEq for Material<P>
where
    P: SharedPointerKind,
{
    fn eq(&self, other: &Self) -> bool {
        let color = Shared::ptr_eq(&self.color, &other.color) as u8;
        let normal = Shared::ptr_eq(&self.normal, &other.normal) as u8;
        let metal_rough = Shared::ptr_eq(&self.metal_rough, &other.metal_rough) as u8;

        color * normal * metal_rough == 1
    }
}

impl<P> PartialOrd for Material<P>
where
    P: SharedPointerKind,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Container of a shared `Model` reference and the optional filter or pose that might be used
/// during rendering.
#[derive(Debug)]
#[non_exhaustive]
pub struct Mesh<P>
where
    P: 'static + SharedPointerKind,
{
    /// The mesh or meshes, if set, to render from within the larger collection of meshes this model
    /// may contain.
    ///
    /// `MeshFilter` instances are acquired from the `Model::filter(&self, ...)` function and can
    /// only be used with the same model they are retrieved from.
    pub filter: Option<MeshFilter>,

    /// The shared model reference to render.
    pub model: Shared<Model<P>, P>,

    /// The animation pose, if set, to use while rendering this model.
    pub pose: Option<Pose>,
}

impl<M, P> From<M> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from(model: M) -> Self {
        Self {
            filter: None,
            model: model.into(),
            pose: None,
        }
    }
}

impl<M, P> From<(M, MeshFilter)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, filter): (M, MeshFilter)) -> Self {
        Self {
            filter: Some(filter),
            model: model.into(),
            pose: None,
        }
    }
}

impl<M, P> From<(M, Option<MeshFilter>)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, filter): (M, Option<MeshFilter>)) -> Self {
        Self {
            filter,
            model: model.into(),
            pose: None,
        }
    }
}

impl<M, P> From<(M, Pose)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, pose): (M, Pose)) -> Self {
        Self {
            filter: None,
            model: model.into(),
            pose: Some(pose),
        }
    }
}

impl<M, P> From<(M, Option<Pose>)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, pose): (M, Option<Pose>)) -> Self {
        Self {
            filter: None,
            model: model.into(),
            pose,
        }
    }
}

impl<M, P> From<(M, MeshFilter, Pose)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, filter, pose): (M, MeshFilter, Pose)) -> Self {
        Self {
            filter: Some(filter),
            model: model.into(),
            pose: Some(pose),
        }
    }
}

impl<M, P> From<(M, Option<MeshFilter>, Option<Pose>)> for Mesh<P>
where
    M: Into<Shared<Model<P>, P>>,
    P: SharedPointerKind,
{
    fn from((model, filter, pose): (M, Option<MeshFilter>, Option<Pose>)) -> Self {
        Self {
            filter,
            model: model.into(),
            pose,
        }
    }
}

/// Description of a model, which may be posed or filtered.
#[derive(Debug)]
#[non_exhaustive]
pub struct ModelCommand<P>
where
    P: 'static + SharedPointerKind,
{
    // TODO: Could probably be u16?
    pub(super) camera_order: f32,

    /// The material to use while rendering this model.
    pub material: Material<P>,

    /// The mesh or meshes, if set, to render from within the larger collection of meshes this model
    /// may contain.
    ///
    /// `MeshFilter` instances are acquired from the `Model::filter(&self, ...)` function and can
    /// only be used with the same model they are retrieved from.
    pub mesh_filter: Option<MeshFilter>,

    /// The shared model reference to render.
    pub model: Shared<Model<P>, P>,

    /// The animation pose, if set, to use while rendering this model.
    pub pose: Option<Pose>,

    /// The generalized matrix transform usually used to desctibe translation, scale and rotation.
    ///
    /// _NOTE_: This is the "world" matrix; view and projection matrices are handled automatically.
    pub transform: Mat4,
}

impl<P> ModelCommand<P>
where
    P: SharedPointerKind,
{
    /// Returns a tightly fitting sphere around the model.
    ///
    /// Includes the animation pose, if specified.
    pub fn bounds(&self) -> Sphere {
        let bounds = if let Some(pose) = &self.pose {
            self.model.pose_bounds(pose)
        } else {
            self.model.bounds()
        };

        bounds.transform(self.transform)
    }
}

impl<P> Clone for ModelCommand<P>
where
    P: SharedPointerKind,
{
    fn clone(&self) -> Self {
        Self {
            camera_order: self.camera_order,
            material: self.material.clone(),
            mesh_filter: self.mesh_filter,
            model: Shared::clone(&self.model),
            pose: self.pose.clone(),
            transform: self.transform,
        }
    }
}

/// Description of a point light shining on models.
#[derive(Clone, Debug, Default)]
pub struct PointLightCommand {
    /// The location of the center of this light in world space.
    pub center: Vec3,

    /// Color of the projected light.
    pub color: Color,

    /// Scalar light "power" value, modelled after lumens but not realistically.
    pub lumens: f32,

    /// Distance from `center` to the furthest reach of the point light.
    pub radius: f32,
}

impl PointLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this light.
    pub fn bounds(&self) -> Sphere {
        Sphere::new(self.center, self.radius)
    }
}

/// Description of a rectangular light shining on models.
///
/// The lit area forms a [square frustum](https://en.wikipedia.org/wiki/Frustum).
#[derive(Clone, Debug, Default)]
pub struct RectLightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// The width and height of the rectangular light, when viewed from above (the unlit side).
    pub dims: CoordF,

    /// Scalar light "power" value, modelled after lumens but not realistically.
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

    pub(super) scale: f32,
}

impl RectLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this light.
    pub fn bounds(&self) -> Sphere {
        todo!();
    }
}

/// Description of a spotlight shining on models.
#[derive(Clone, Debug, Default)]
pub struct SpotlightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// Scalar light "power" value, modelled after lumens but not realistically.
    pub lumens: f32,

    /// Direction of the light rays.
    pub normal: Vec3,

    /// Position of the light source.
    pub position: Vec3,

    /// Radius of the spotlight cone.
    pub radius: f32,

    pub(crate) radius_start: f32,

    /// Lit distance from `position` and to the bottom of the spotlight.
    pub range: Range<f32>,

    pub(super) scale: f32,
}

impl SpotlightCommand {
    // TODO: Maybe just use a sphere. Heyyyyyyy, benchmarks!!!!! Yay!!!
    /// Returns a tightly fitting cone around the lit area of this light.
    pub fn bounds(&self) -> Cone {
        Cone::new(self.position, self.normal, self.range.end, self.radius)
    }
}

/// Description of sunlight shining on models.
#[derive(Clone, Debug, Default)]
pub struct SunlightCommand {
    /// Color of the projected light.
    pub color: Color,

    /// Scalar light "power" value, modelled after lumens but not realistically.
    pub lumens: f32,

    /// Direction of the light rays.
    pub normal: Vec3,
}
