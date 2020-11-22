use {
    super::{
        LineCommand, LineVertex, Material, MeshCommand, PointLightCommand, RectLightCommand,
        SpotlightCommand, SunlightCommand,
    },
    crate::{
        color::{AlphaColor, Color},
        gpu::ModelRef,
        math::{vec3_is_finite, Mat4, Vec3},
    },
    std::{num::FpCategory, ops::Range},
};

/// An expressive type which allows specification of individual draws.
#[derive(Clone)]
pub enum Command {
    Line(LineCommand),
    Mesh(MeshCommand),
    PointLight(PointLightCommand),
    RectLight(RectLightCommand),
    Spotlight(SpotlightCommand),
    Sunlight(SunlightCommand),
}

impl Command {
    pub(crate) fn as_line(&self) -> Option<&LineCommand> {
        match self {
            Self::Line(res) => Some(res),
            _ => None,
        }
    }

    pub(crate) fn as_mesh(&self) -> Option<&MeshCommand> {
        match self {
            Self::Mesh(res) => Some(res),
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

    pub(crate) fn is_mesh(&self) -> bool {
        self.as_mesh().is_some()
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

    /// Draws a line between the given coordinates using a constant width and two colors. The colors specify a gradient if
    /// they differ. Generally intended to support debugging use cases such as drawing bounding boxes.
    pub fn line<S: Into<Vec3>, SC: Into<AlphaColor>, E: Into<Vec3>, EC: Into<AlphaColor>>(
        start: S,
        start_color: SC,
        end: E,
        end_color: EC,
    ) -> Self {
        Self::Line(LineCommand([
            LineVertex {
                color: start_color.into(),
                pos: start.into(),
            },
            LineVertex {
                color: end_color.into(),
                pos: end.into(),
            },
        ]))
    }

    pub fn mesh<M: Into<Mesh>>(mesh: M, material: Material, transform: Mat4) -> Self {
        let mesh = mesh.into();
        Self::Mesh(MeshCommand {
            camera_z: f32::NAN,
            material,
            name_filter: mesh.name_filter,
            model: mesh.model,
            skin: mesh.skin,
            transform,
        })
    }

    /// Draws a spotlight with the given color, position/orientation, and shape.
    ///
    /// _Note_: Spotlights have a hemispherical cap on the bottom, so a given `range` will be the maximum range
    /// and you may not see any light on objects at that distance. Move the light a bit towards the object to
    /// enter the penumbra.
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
        power: f32, // TODO: color+power look like they should just be an sRGB type?
        pos: Vec3,
        normal: Vec3,
        range: Range<f32>,
        cone_angle: f32,
        penumbra_angle: f32,
    ) -> Self {
        assert_eq!(power.classify(), FpCategory::Normal);
        assert!(vec3_is_finite(pos));
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
            normal,
            penumbra_radius,
            pos,
            power,
            range,
            top_radius,
        })
    }
}

// impl SunlightCommand {
//     fn new<C>(camera: C, e: &Sunlight) -> Self
//     where
//         C: Camera,
//     {
//         let view_inv = camera.view_inv();

//         // TODO: Calculate this with object AABBs once those are ready (any AABB inside both the camera and shadow projections)
//         // Calculate the world-space coords of the eight points that make up our camera frustum
//         // and calculate the min/max/mid coordinates of them
//         let camera_world = [
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, 1.0)), 1.0))
//                 .truncate(),
//         ];
//         let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
//             let p0 = camera_world[0];
//             (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
//         };
//         for pi in &camera_world {
//             min_x = pi.x().min(min_x);
//             min_y = pi.y().min(min_y);
//             min_z = pi.z().min(min_z);
//             max_x = pi.x().max(max_x);
//             max_y = pi.y().max(max_y);
//             max_z = pi.z().max(max_z);
//         }
//         let mid_x = (max_x + min_x) / 2.0;
//         let mid_y = (max_y + min_y) / 2.0;
//         let mid_z = (max_z + min_z) / 2.0;
//         let position = vec3(mid_x, mid_y, mid_z);
//         let target = position + e.normal;
//         let n_dot_x = e.normal.dot(Vec3::unit_x()).abs();
//         let n_dot_y = e.normal.dot(Vec3::unit_y()).abs();
//         let up = if n_dot_x < n_dot_y {
//             Vec3::unit_x()
//         } else {
//             Vec3::unit_y()
//         };
//         let light_view = Mat4::look_at_rh(position, target, up);
//         let light_world = [
//             (light_view * vec4_from_vec3(camera_world[0], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[1], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[2], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[3], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[4], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[5], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[6], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[7], 1.0)).truncate(),
//         ];
//         let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
//             let p0 = light_world[0];
//             (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
//         };
//         for pi in &light_world {
//             min_x = pi.x().min(min_x);
//             min_y = pi.y().min(min_y);
//             min_z = pi.z().min(min_z);
//             max_x = pi.x().max(max_x);
//             max_y = pi.y().max(max_y);
//             max_z = pi.z().max(max_z);
//         }
//         let light_space =
//             Mat4::orthographic_rh(min_x, max_x, min_y, max_y, min_z, max_z) * light_view;

//         Self {
//             normal_inv: -e.normal,
//             diffuse: e.diffuse,
//             power: e.power,
//             light_space,
//         }
//     }
// }

// impl SpotlightCommand {
//     fn new() -> Self {
//         //             let up = Vec3::unit_z();
//         //             let light_view = Mat4::look_at_rh(e.position, e.position + e.normal, up);
//         //             let light_space =
//         //                 Mat4::perspective_rh_gl(2.0 * e.cutoff_outer, 1.0, 1.0, 35.0) * light_view;
//         //             let cutoff_inner = e.cutoff_inner.cos();
//         //             let cutoff_outer = e.cutoff_outer.cos();
//         //             draw_commands.push(
//         //                 SpotlightCommand {
//         //                     anormal: -e.normal,
//         //                     cutoff_inner,
//         //                     cutoff_outer,
//         //                     diffuse: e.diffuse,
//         //                     position: e.position,
//         //                     power: e.power,
//         //                     light_space,
//         //                 }
//         //                 .into(),
//         //             );

//         todo!();
//     }
// }

pub struct Mesh {
    model: ModelRef,
    name_filter: Option<Option<&'static str>>,
    skin: Option<u8>,
}

impl From<ModelRef> for Mesh {
    fn from(model: ModelRef) -> Self {
        Self {
            model,
            name_filter: None,
            skin: None,
        }
    }
}

impl From<(ModelRef, Option<&'static str>)> for Mesh {
    fn from((model, name_filter): (ModelRef, Option<&'static str>)) -> Self {
        Self {
            model,
            name_filter: Some(name_filter),
            skin: None,
        }
    }
}

impl From<(ModelRef, Option<&'static str>, u8)> for Mesh {
    fn from((model, name_filter, skin): (ModelRef, Option<&'static str>, u8)) -> Self {
        Self {
            model,
            name_filter: Some(name_filter),
            skin: Some(skin),
        }
    }
}
