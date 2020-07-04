use crate::{
    color::Color,
    math::{Mat4, Vec3},
};

#[derive(Debug)]
pub struct SpotlightCommand {
    normal_inv: Vec3,
    cutoff_inner: f32,
    cutoff_outer: f32,
    diffuse: Color,
    position: Vec3,
    power: f32,
    light_space: Mat4,
}

impl SpotlightCommand {
    fn new() -> Self {
        //             let up = Vec3::unit_z();
        //             let light_view = Mat4::look_at_rh(e.position, e.position + e.normal, up);
        //             let light_space =
        //                 Mat4::perspective_rh_gl(2.0 * e.cutoff_outer, 1.0, 1.0, 35.0) * light_view;
        //             let cutoff_inner = e.cutoff_inner.cos();
        //             let cutoff_outer = e.cutoff_outer.cos();
        //             draw_commands.push(
        //                 SpotlightCommand {
        //                     anormal: -e.normal,
        //                     cutoff_inner,
        //                     cutoff_outer,
        //                     diffuse: e.diffuse,
        //                     position: e.position,
        //                     power: e.power,
        //                     light_space,
        //                 }
        //                 .into(),
        //             );

        todo!();
    }
}
