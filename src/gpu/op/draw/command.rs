use {
    super::{LineCommand, LineVertex, Material},
    crate::{
        color::{AlphaColor, Color},
        gpu::{MeshFilter, ModelRef, Pose},
        math::{vec3_is_finite, CoordF, Mat4, Sphere, Vec3},
    },
    std::{num::FpCategory, ops::Range},
};

/// An expressive type which allows specification of individual draws.
pub enum Command {
    Line(LineCommand),
    Model(ModelCommand),
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

    pub fn point_light(center: Vec3, color: Color, power: f32, radius: f32) -> Self {
        Self::PointLight(PointLightCommand {
            center,
            color,
            power,
            radius,
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

#[derive(Debug)]
pub struct Mesh {
    pub filter: Option<MeshFilter>,
    pub model: ModelRef,
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

impl From<(ModelRef, Pose)> for Mesh {
    fn from((model, pose): (ModelRef, Pose)) -> Self {
        Self {
            filter: None,
            model,
            pose: Some(pose),
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

#[derive(Debug)]
pub struct ModelCommand {
    pub(super) camera_order: f32, // TODO: Could probably be u16?
    pub material: Material,
    pub mesh_filter: Option<MeshFilter>,
    pub model: ModelRef,
    pub pose: Option<Pose>,
    pub transform: Mat4,
}

#[derive(Clone, Debug)]
pub struct PointLightCommand {
    pub center: Vec3,
    pub color: Color,
    pub power: f32,
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct RectLightCommand {
    pub color: Color, // full-bright and penumbra-to-transparent color
    pub dims: CoordF,
    pub radius: f32, // size of the penumbra area beyond the box formed by `pos` and `range` which fades from `color` to transparent
    pub pos: Vec3,   // top-left corner when viewed from above
    pub power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
    pub range: f32, // distance from `pos` to the bottom of the rectangular light
}

impl RectLightCommand {
    /// Returns a tightly fitting sphere around the lit area of this rectangular light, including the penumbra
    pub(self) fn bounds(&self) -> Sphere {
        todo!();
    }
}

#[derive(Clone, Debug)]
pub struct SunlightCommand {
    pub color: Color, // uniform color for any area exposed to the sunlight
    pub normal: Vec3, // direction which the sunlight shines
    pub power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
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

#[derive(Clone, Debug)]
pub struct SpotlightCommand {
    pub color: Color,         // `cone` and penumbra-to-transparent color
    pub cone_radius: f32, // radius of the spotlight cone from the center to the edge of the full-bright area
    pub normal: Vec3,     // direction from `pos` which the spotlight shines
    pub penumbra_radius: f32, // Additional radius beyond `cone_radius` which fades from `color` to transparent
    pub pos: Vec3,            // position of the pointy end
    pub power: f32, // sRGB power value, normalized to current gamma so 1.0 == a user setting of 1.2 and 2.0 == 2.4
    pub range: Range<f32>, // lit distance from `pos` and to the bottom of the spotlight (does not account for the lens-shaped end)
    pub top_radius: f32,
}

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
// impl SpotlightCommand {
//     /// Returns a tightly fitting cone around the lit area of this spotlight, including the penumbra and
//     /// lens-shaped base.
//     pub(self) fn bounds(&self) -> Cone {
//         Cone::new(
//             self.pos,
//             self.normal,
//             self.range.end,
//             self.cone_radius + self.penumbra_radius,
//         )
//     }
// }
